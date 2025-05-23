mod app_menus;
pub mod inline_completion_registry;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(crate) mod linux_prompts;
#[cfg(target_os = "macos")]
pub(crate) mod mac_only_instance;
mod open_listener;
mod quick_action_bar;
#[cfg(target_os = "windows")]
pub(crate) mod windows_only_instance;

use anyhow::Context as _;
pub use app_menus::*;
use assets::Assets;
use assistant::PromptBuilder;
use breadcrumbs::Breadcrumbs;
use client::{zed_urls, ZED_URL_SCHEME};
use collections::VecDeque;
use command_palette_hooks::CommandPaletteFilter;
use editor::ProposedChangesEditorToolbar;
use editor::{scroll::Autoscroll, Editor, MultiBuffer};
use feature_flags::FeatureFlagAppExt;
use futures::{channel::mpsc, select_biased, StreamExt};
use gpui::{
    actions, point, px, AppContext, AsyncAppContext, Context, FocusableView, MenuItem,
    PathPromptOptions, PromptLevel, ReadGlobal, Task, TitlebarOptions, View, ViewContext,
    VisualContext, WindowKind, WindowOptions,
};
pub use open_listener::*;
use outline_panel::OutlinePanel;
use paths::{local_settings_file_relative_path, local_tasks_file_relative_path};
use project::{DirectoryLister, ProjectItem};
use project_panel::ProjectPanel;
use quick_action_bar::QuickActionBar;
use recent_projects::open_ssh_project;
use release_channel::{AppCommitSha, ReleaseChannel};
use rope::Rope;
use search::project_search::ProjectSearchBar;
use settings::{
    initial_project_settings_content, initial_tasks_content, KeymapFile, Settings, SettingsStore,
    DEFAULT_KEYMAP_PATH,
};
use std::any::TypeId;
use std::path::PathBuf;
use std::{borrow::Cow, ops::Deref, path::Path, sync::Arc};
use terminal_view::terminal_panel::{self, TerminalPanel};
use theme::ActiveTheme;
use util::{asset_str, ResultExt};
use uuid::Uuid;
//use vim_mode_setting::VimModeSetting;
use welcome::{BaseKeymap, MultibufferHint};
use workspace::notifications::NotificationId;
use workspace::CloseIntent;
use workspace::{
    create_and_open_local_file, notifications::simple_message_notification::MessageNotification,
    open_new, AppState, NewFile, NewWindow, OpenLog, Toast, Workspace, WorkspaceSettings,
};
use workspace::{notifications::DetachAndPromptErr, Pane};
use zed_actions::{
    OpenAccountSettings, OpenBrowser, OpenServerSettings, OpenSettings, OpenZedUrl, Quit,
};

actions!(
    zed,
    [
        DebugElements,
        Hide,
        HideOthers,
        Minimize,
        OpenDefaultSettings,
        OpenProjectSettings,
        OpenProjectTasks,
        OpenTasks,
        ResetDatabase,
        ShowAll,
        ToggleFullScreen,
        Zoom,
        TestPanic,
    ]
);

pub fn init(cx: &mut AppContext) {
    #[cfg(target_os = "macos")]
    cx.on_action(|_: &Hide, cx| cx.hide());
    #[cfg(target_os = "macos")]
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    #[cfg(target_os = "macos")]
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(quit);

    if ReleaseChannel::global(cx) == ReleaseChannel::Dev {
        cx.on_action(test_panic);
    }
}

pub fn build_window_options(display_uuid: Option<Uuid>, cx: &mut AppContext) -> WindowOptions {
    let display = display_uuid.and_then(|uuid| {
        cx.displays()
            .into_iter()
            .find(|display| display.uuid().ok() == Some(uuid))
    });
    let app_id = ReleaseChannel::global(cx).app_id();
    let window_decorations = match std::env::var("ZED_WINDOW_DECORATIONS") {
        Ok(val) if val == "server" => gpui::WindowDecorations::Server,
        Ok(val) if val == "client" => gpui::WindowDecorations::Client,
        _ => gpui::WindowDecorations::Client,
    };

    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: Some(point(px(9.0), px(9.0))),
        }),
        window_bounds: None,
        focus: false,
        show: false,
        kind: WindowKind::Normal,
        is_movable: true,
        display_id: display.map(|display| display.id()),
        window_background: cx.theme().window_background_appearance(),
        app_id: Some(app_id.to_owned()),
        window_decorations: Some(window_decorations),
        window_min_size: Some(gpui::Size {
            width: px(360.0),
            height: px(240.0),
        }),
    }
}

pub fn initialize_workspace(
    app_state: Arc<AppState>,
    prompt_builder: Arc<PromptBuilder>,
    cx: &mut AppContext,
) {
    cx.observe_new_views(move |workspace: &mut Workspace, cx| {
        let workspace_handle = cx.view().clone();
        let center_pane = workspace.active_pane().clone();
        initialize_pane(workspace, &center_pane, cx);
        cx.subscribe(&workspace_handle, {
            move |workspace, _, event, cx| match event {
                workspace::Event::PaneAdded(pane) => {
                    initialize_pane(workspace, pane, cx);
                }
                workspace::Event::OpenBundledFile {
                    text,
                    title,
                    language,
                } => open_bundled_file(workspace, text.clone(), title, language, cx),
                _ => {}
            }
        })
        .detach();

        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        initialize_linux_file_watcher(cx);

        if let Some(specs) = cx.gpu_specs() {
            log::info!("Using GPU: {:?}", specs);
            show_software_emulation_warning_if_needed(specs, cx);
        }

        let inline_completion_button = cx.new_view(|cx| {
            inline_completion_button::InlineCompletionButton::new(
                workspace.weak_handle(),
                app_state.fs.clone(),
                cx,
            )
        });

        let diagnostic_summary =
            cx.new_view(|cx| diagnostics::items::DiagnosticIndicator::new(workspace, cx));
        let activity_indicator =
            activity_indicator::ActivityIndicator::new(workspace, app_state.languages.clone(), cx);
        let active_buffer_language =
            cx.new_view(|_| language_selector::ActiveBufferLanguage::new(workspace));
        let active_toolchain_language =
            cx.new_view(|cx| toolchain_selector::ActiveToolchain::new(workspace, cx));
        //let vim_mode_indicator = cx.new_view(vim::ModeIndicator::new);
        let cursor_position =
            cx.new_view(|_| go_to_line::cursor_position::CursorPosition::new(workspace));
        workspace.status_bar().update(cx, |status_bar, cx| {
            status_bar.add_left_item(diagnostic_summary, cx);
            status_bar.add_left_item(activity_indicator, cx);
            status_bar.add_right_item(inline_completion_button, cx);
            status_bar.add_right_item(active_buffer_language, cx);
            status_bar.add_right_item(active_toolchain_language, cx);
            //status_bar.add_right_item(vim_mode_indicator, cx);
            status_bar.add_right_item(cursor_position, cx);
        });

        auto_update_ui::notify_of_any_new_update(cx);

        let handle = cx.view().downgrade();
        cx.on_window_should_close(move |cx| {
            handle
                .update(cx, |workspace, cx| {
                    // We'll handle closing asynchronously
                    workspace.close_window(&Default::default(), cx);
                    false
                })
                .unwrap_or(true)
        });

        initialize_panels(prompt_builder.clone(), cx);
        register_actions(app_state.clone(), workspace, cx);

        workspace.focus_handle(cx).focus(cx);
    })
    .detach();

    feature_gate_zed_pro_actions(cx);
}

fn feature_gate_zed_pro_actions(cx: &mut AppContext) {
    let zed_pro_actions = [TypeId::of::<OpenAccountSettings>()];

    CommandPaletteFilter::update_global(cx, |filter, _cx| {
        filter.hide_action_types(&zed_pro_actions);
    });

    cx.observe_flag::<feature_flags::ZedPro, _>({
        move |is_enabled, cx| {
            CommandPaletteFilter::update_global(cx, |filter, _cx| {
                if is_enabled {
                    filter.show_action_types(zed_pro_actions.iter());
                } else {
                    filter.hide_action_types(&zed_pro_actions);
                }
            });
        }
    })
    .detach();
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn initialize_linux_file_watcher(cx: &mut ViewContext<Workspace>) {
    if let Err(e) = fs::linux_watcher::global(|_| {}) {
        let message = format!(
            db::indoc! {r#"
            inotify_init returned {}

            This may be due to system-wide limits on inotify instances. For troubleshooting see: https://zed.dev/docs/linux
            "#},
            e
        );
        let prompt = cx.prompt(
            PromptLevel::Critical,
            "Could not start inotify",
            Some(&message),
            &["Troubleshoot and Quit"],
        );
        cx.spawn(|_, mut cx| async move {
            if prompt.await == Ok(0) {
                cx.update(|cx| {
                    cx.open_url("https://zed.dev/docs/linux#could-not-start-inotify");
                    cx.quit();
                })
                .ok();
            }
        })
        .detach()
    }
}

fn show_software_emulation_warning_if_needed(
    specs: gpui::GpuSpecs,
    cx: &mut ViewContext<Workspace>,
) {
    if specs.is_software_emulated && std::env::var("ZED_ALLOW_EMULATED_GPU").is_err() {
        let message = format!(
            db::indoc! {r#"
            Zed uses Vulkan for rendering and requires a compatible GPU.

            Currently you are using a software emulated GPU ({}) which
            will result in awful performance.

            For troubleshooting see: https://zed.dev/docs/linux
            Set ZED_ALLOW_EMULATED_GPU=1 env var to permanently override.
            "#},
            specs.device_name
        );
        let prompt = cx.prompt(
            PromptLevel::Critical,
            "Unsupported GPU",
            Some(&message),
            &["Skip", "Troubleshoot and Quit"],
        );
        cx.spawn(|_, mut cx| async move {
            if prompt.await == Ok(1) {
                cx.update(|cx| {
                    cx.open_url("https://zed.dev/docs/linux#zed-fails-to-open-windows");
                    cx.quit();
                })
                .ok();
            }
        })
        .detach()
    }
}

fn initialize_panels(prompt_builder: Arc<PromptBuilder>, cx: &mut ViewContext<Workspace>) {
    let assistant2_feature_flag = cx.wait_for_flag::<feature_flags::Assistant2FeatureFlag>();
    let git_ui_feature_flag = cx.wait_for_flag::<feature_flags::GitUiFeatureFlag>();

    let prompt_builder = prompt_builder.clone();

    cx.spawn(|workspace_handle, mut cx| async move {
        let project_panel = ProjectPanel::load(workspace_handle.clone(), cx.clone());
        let outline_panel = OutlinePanel::load(workspace_handle.clone(), cx.clone());
        let terminal_panel = TerminalPanel::load(workspace_handle.clone(), cx.clone());
        let channels_panel =
            collab_ui::collab_panel::CollabPanel::load(workspace_handle.clone(), cx.clone());
        let chat_panel =
            collab_ui::chat_panel::ChatPanel::load(workspace_handle.clone(), cx.clone());
        let notification_panel = collab_ui::notification_panel::NotificationPanel::load(
            workspace_handle.clone(),
            cx.clone(),
        );
        let assistant_panel =
            assistant::AssistantPanel::load(workspace_handle.clone(), prompt_builder, cx.clone());

        let (
            project_panel,
            outline_panel,
            terminal_panel,
            channels_panel,
            chat_panel,
            notification_panel,
            assistant_panel,
        ) = futures::try_join!(
            project_panel,
            outline_panel,
            terminal_panel,
            channels_panel,
            chat_panel,
            notification_panel,
            assistant_panel,
        )?;

        workspace_handle.update(&mut cx, |workspace, cx| {
            workspace.add_panel(project_panel, cx);
            workspace.add_panel(outline_panel, cx);
            workspace.add_panel(terminal_panel, cx);
            workspace.add_panel(channels_panel, cx);
            workspace.add_panel(chat_panel, cx);
            workspace.add_panel(notification_panel, cx);
            workspace.add_panel(assistant_panel, cx)
        })?;

        let git_ui_enabled = git_ui_feature_flag.await;
        let git_panel = if git_ui_enabled {
            Some(git_ui::git_panel::GitPanel::load(workspace_handle.clone(), cx.clone()).await?)
        } else {
            None
        };
        workspace_handle.update(&mut cx, |workspace, cx| {
            if let Some(git_panel) = git_panel {
                workspace.add_panel(git_panel, cx);
            }
        })?;

        let is_assistant2_enabled = if cfg!(test) {
            false
        } else {
            assistant2_feature_flag.await
        };
        let assistant2_panel = if is_assistant2_enabled {
            Some(assistant2::AssistantPanel::load(workspace_handle.clone(), cx.clone()).await?)
        } else {
            None
        };
        workspace_handle.update(&mut cx, |workspace, cx| {
            if let Some(assistant2_panel) = assistant2_panel {
                workspace.add_panel(assistant2_panel, cx);
            }

            if is_assistant2_enabled {
                workspace.register_action(assistant2::InlineAssistant::inline_assist);
            } else {
                workspace.register_action(assistant::AssistantPanel::inline_assist);
            }
        })?;

        anyhow::Ok(())
    })
    .detach();
}

fn register_actions(
    app_state: Arc<AppState>,
    workspace: &mut Workspace,
    cx: &mut ViewContext<Workspace>,
) {
    workspace
        .register_action(about)
        .register_action(|_, _: &Minimize, cx| {
            cx.minimize_window();
        })
        .register_action(|_, _: &Zoom, cx| {
            cx.zoom_window();
        })
        .register_action(|_, _: &ToggleFullScreen, cx| {
            cx.toggle_fullscreen();
        })
        .register_action(|_, action: &OpenZedUrl, cx| {
            OpenListener::global(cx).open_urls(vec![action.url.clone()])
        })
        .register_action(|_, action: &OpenBrowser, cx| cx.open_url(&action.url))
        .register_action(move |_, _: &zed_actions::IncreaseBufferFontSize, cx| {
            theme::adjust_buffer_font_size(cx, |size| *size += px(1.0))
        })
        .register_action(|workspace, _: &workspace::Open, cx| {
            workspace
                .client()
                .telemetry()
                .report_app_event("open project".to_string());
            let paths = workspace.prompt_for_open_path(
                PathPromptOptions {
                    files: true,
                    directories: true,
                    multiple: true,
                },
                DirectoryLister::Project(workspace.project().clone()),
                cx,
            );

            cx.spawn(|this, mut cx| async move {
                let Some(paths) = paths.await.log_err().flatten() else {
                    return;
                };

                if let Some(task) = this
                    .update(&mut cx, |this, cx| {
                        if this.project().read(cx).is_local() {
                            this.open_workspace_for_paths(false, paths, cx)
                        } else {
                            open_new_ssh_project_from_project(this, paths, cx)
                        }
                    })
                    .log_err()
                {
                    task.await.log_err();
                }
            })
            .detach()
        })
        .register_action(move |_, _: &zed_actions::DecreaseBufferFontSize, cx| {
            theme::adjust_buffer_font_size(cx, |size| *size -= px(1.0))
        })
        .register_action(move |_, _: &zed_actions::ResetBufferFontSize, cx| {
            theme::reset_buffer_font_size(cx)
        })
        .register_action(move |_, _: &zed_actions::IncreaseUiFontSize, cx| {
            theme::adjust_ui_font_size(cx, |size| *size += px(1.0))
        })
        .register_action(move |_, _: &zed_actions::DecreaseUiFontSize, cx| {
            theme::adjust_ui_font_size(cx, |size| *size -= px(1.0))
        })
        .register_action(move |_, _: &zed_actions::ResetUiFontSize, cx| {
            theme::reset_ui_font_size(cx)
        })
        .register_action(move |_, _: &zed_actions::IncreaseBufferFontSize, cx| {
            theme::adjust_buffer_font_size(cx, |size| *size += px(1.0))
        })
        .register_action(move |_, _: &zed_actions::DecreaseBufferFontSize, cx| {
            theme::adjust_buffer_font_size(cx, |size| *size -= px(1.0))
        })
        .register_action(move |_, _: &zed_actions::ResetBufferFontSize, cx| {
            theme::reset_buffer_font_size(cx)
        })
        .register_action(install_cli)
        .register_action(|_, _: &install_cli::RegisterZedScheme, cx| {
            cx.spawn(|workspace, mut cx| async move {
                register_zed_scheme(&cx).await?;
                workspace.update(&mut cx, |workspace, cx| {
                    struct RegisterZedScheme;

                    workspace.show_toast(
                        Toast::new(
                            NotificationId::unique::<RegisterZedScheme>(),
                            format!(
                                "zed:// links will now open in {}.",
                                ReleaseChannel::global(cx).display_name()
                            ),
                        ),
                        cx,
                    )
                })?;
                Ok(())
            })
            .detach_and_prompt_err("Error registering zed:// scheme", cx, |_, _| None);
        })
        .register_action(|workspace, _: &OpenLog, cx| {
            open_log_file(workspace, cx);
        })
        .register_action(|workspace, _: &zed_actions::OpenLicenses, cx| {
            open_bundled_file(
                workspace,
                asset_str::<Assets>("licenses.md"),
                "Open Source License Attribution",
                "Markdown",
                cx,
            );
        })
        .register_action(
            move |workspace: &mut Workspace,
                  _: &zed_actions::OpenTelemetryLog,
                  cx: &mut ViewContext<Workspace>| {
                open_telemetry_log_file(workspace, cx);
            },
        )
        .register_action(
            move |_: &mut Workspace,
                  _: &zed_actions::OpenKeymap,
                  cx: &mut ViewContext<Workspace>| {
                open_settings_file(
                    paths::keymap_file(),
                    || settings::initial_keymap_content().as_ref().into(),
                    cx,
                );
            },
        )
        .register_action(
            move |_: &mut Workspace, _: &OpenSettings, cx: &mut ViewContext<Workspace>| {
                open_settings_file(
                    paths::settings_file(),
                    || settings::initial_user_settings_content().as_ref().into(),
                    cx,
                );
            },
        )
        .register_action(
            |_: &mut Workspace, _: &OpenAccountSettings, cx: &mut ViewContext<Workspace>| {
                cx.open_url(&zed_urls::account_url(cx));
            },
        )
        .register_action(
            move |_: &mut Workspace, _: &OpenTasks, cx: &mut ViewContext<Workspace>| {
                open_settings_file(
                    paths::tasks_file(),
                    || settings::initial_tasks_content().as_ref().into(),
                    cx,
                );
            },
        )
        .register_action(open_project_settings_file)
        .register_action(open_project_tasks_file)
        .register_action(
            move |workspace: &mut Workspace,
                  _: &zed_actions::OpenDefaultKeymap,
                  cx: &mut ViewContext<Workspace>| {
                open_bundled_file(
                    workspace,
                    settings::default_keymap(),
                    "Default Key Bindings",
                    "JSON",
                    cx,
                );
            },
        )
        .register_action(
            move |workspace: &mut Workspace,
                  _: &OpenDefaultSettings,
                  cx: &mut ViewContext<Workspace>| {
                open_bundled_file(
                    workspace,
                    settings::default_settings(),
                    "Default Settings",
                    "JSON",
                    cx,
                );
            },
        )
        .register_action(
            |workspace: &mut Workspace,
             _: &project_panel::ToggleFocus,
             cx: &mut ViewContext<Workspace>| {
                workspace.toggle_panel_focus::<ProjectPanel>(cx);
            },
        )
        .register_action(
            |workspace: &mut Workspace,
             _: &outline_panel::ToggleFocus,
             cx: &mut ViewContext<Workspace>| {
                workspace.toggle_panel_focus::<OutlinePanel>(cx);
            },
        )
        .register_action(
            |workspace: &mut Workspace,
             _: &collab_ui::collab_panel::ToggleFocus,
             cx: &mut ViewContext<Workspace>| {
                workspace.toggle_panel_focus::<collab_ui::collab_panel::CollabPanel>(cx);
            },
        )
        .register_action(
            |workspace: &mut Workspace,
             _: &collab_ui::chat_panel::ToggleFocus,
             cx: &mut ViewContext<Workspace>| {
                workspace.toggle_panel_focus::<collab_ui::chat_panel::ChatPanel>(cx);
            },
        )
        .register_action(
            |workspace: &mut Workspace,
             _: &collab_ui::notification_panel::ToggleFocus,
             cx: &mut ViewContext<Workspace>| {
                workspace
                    .toggle_panel_focus::<collab_ui::notification_panel::NotificationPanel>(cx);
            },
        )
        .register_action(
            |workspace: &mut Workspace,
             _: &terminal_panel::ToggleFocus,
             cx: &mut ViewContext<Workspace>| {
                workspace.toggle_panel_focus::<TerminalPanel>(cx);
            },
        )
        .register_action({
            let app_state = Arc::downgrade(&app_state);
            move |_, _: &NewWindow, cx| {
                if let Some(app_state) = app_state.upgrade() {
                    open_new(Default::default(), app_state, cx, |workspace, cx| {
                        Editor::new_file(workspace, &Default::default(), cx)
                    })
                    .detach();
                }
            }
        })
        .register_action({
            let app_state = Arc::downgrade(&app_state);
            move |_, _: &NewFile, cx| {
                if let Some(app_state) = app_state.upgrade() {
                    open_new(Default::default(), app_state, cx, |workspace, cx| {
                        Editor::new_file(workspace, &Default::default(), cx)
                    })
                    .detach();
                }
            }
        });
    if workspace.project().read(cx).is_via_ssh() {
        workspace.register_action({
            move |workspace, _: &OpenServerSettings, cx| {
                let open_server_settings = workspace
                    .project()
                    .update(cx, |project, cx| project.open_server_settings(cx));

                cx.spawn(|workspace, mut cx| async move {
                    let buffer = open_server_settings.await?;

                    workspace
                        .update(&mut cx, |workspace, cx| {
                            workspace.open_path(
                                buffer
                                    .read(cx)
                                    .project_path(cx)
                                    .expect("Settings file must have a location"),
                                None,
                                true,
                                cx,
                            )
                        })?
                        .await?;

                    anyhow::Ok(())
                })
                .detach_and_log_err(cx);
            }
        });
    }
}

fn initialize_pane(workspace: &Workspace, pane: &View<Pane>, cx: &mut ViewContext<Workspace>) {
    pane.update(cx, |pane, cx| {
        pane.toolbar().update(cx, |toolbar, cx| {
            let multibuffer_hint = cx.new_view(|_| MultibufferHint::new());
            toolbar.add_item(multibuffer_hint, cx);
            let breadcrumbs = cx.new_view(|_| Breadcrumbs::new());
            toolbar.add_item(breadcrumbs, cx);
            let buffer_search_bar = cx.new_view(search::BufferSearchBar::new);
            toolbar.add_item(buffer_search_bar.clone(), cx);

            let proposed_change_bar = cx.new_view(|_| ProposedChangesEditorToolbar::new());
            toolbar.add_item(proposed_change_bar, cx);
            let quick_action_bar =
                cx.new_view(|cx| QuickActionBar::new(buffer_search_bar, workspace, cx));
            toolbar.add_item(quick_action_bar, cx);
            let diagnostic_editor_controls = cx.new_view(|_| diagnostics::ToolbarControls::new());
            toolbar.add_item(diagnostic_editor_controls, cx);
            let project_search_bar = cx.new_view(|_| ProjectSearchBar::new());
            toolbar.add_item(project_search_bar, cx);
            let lsp_log_item = cx.new_view(|_| language_tools::LspLogToolbarItemView::new());
            toolbar.add_item(lsp_log_item, cx);
            let syntax_tree_item =
                cx.new_view(|_| language_tools::SyntaxTreeToolbarItemView::new());
            toolbar.add_item(syntax_tree_item, cx);
        })
    });
}

fn about(_: &mut Workspace, _: &zed_actions::About, cx: &mut ViewContext<Workspace>) {
    let release_channel = ReleaseChannel::global(cx).display_name();
    let version = env!("CARGO_PKG_VERSION");
    let message = format!("{release_channel} {version}");
    let detail = AppCommitSha::try_global(cx).map(|sha| sha.0.clone());

    let prompt = cx.prompt(PromptLevel::Info, &message, detail.as_deref(), &["OK"]);
    cx.foreground_executor()
        .spawn(async {
            prompt.await.ok();
        })
        .detach();
}

fn test_panic(_: &TestPanic, _: &mut AppContext) {
    panic!("Ran the TestPanic action")
}

fn install_cli(_: &mut Workspace, _: &install_cli::Install, cx: &mut ViewContext<Workspace>) {
    const LINUX_PROMPT_DETAIL: &str = "If you installed Zed from our official release add ~/.local/bin to your PATH.\n\nIf you installed Zed from a different source like your package manager, then you may need to create an alias/symlink manually.\n\nDepending on your package manager, the CLI might be named zeditor, zedit, zed-editor or something else.";

    cx.spawn(|workspace, mut cx| async move {
        if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            let prompt = cx.prompt(
                PromptLevel::Warning,
                "CLI should already be installed",
                Some(LINUX_PROMPT_DETAIL),
                &["Ok"],
            );
            cx.background_executor().spawn(prompt).detach();
            return Ok(());
        }
        let path = install_cli::install_cli(cx.deref())
            .await
            .context("error creating CLI symlink")?;

        workspace.update(&mut cx, |workspace, cx| {
            struct InstalledZedCli;

            workspace.show_toast(
                Toast::new(
                    NotificationId::unique::<InstalledZedCli>(),
                    format!(
                        "Installed `zed` to {}. You can launch {} from your terminal.",
                        path.to_string_lossy(),
                        ReleaseChannel::global(cx).display_name()
                    ),
                ),
                cx,
            )
        })?;
        register_zed_scheme(&cx).await.log_err();
        Ok(())
    })
    .detach_and_prompt_err("Error installing zed cli", cx, |_, _| None);
}

fn quit(_: &Quit, cx: &mut AppContext) {
    let should_confirm = WorkspaceSettings::get_global(cx).confirm_quit;
    cx.spawn(|mut cx| async move {
        let mut workspace_windows = cx.update(|cx| {
            cx.windows()
                .into_iter()
                .filter_map(|window| window.downcast::<Workspace>())
                .collect::<Vec<_>>()
        })?;

        // If multiple windows have unsaved changes, and need a save prompt,
        // prompt in the active window before switching to a different window.
        cx.update(|cx| {
            workspace_windows.sort_by_key(|window| window.is_active(cx) == Some(false));
        })
        .log_err();

        if let (true, Some(workspace)) = (should_confirm, workspace_windows.first().copied()) {
            let answer = workspace
                .update(&mut cx, |_, cx| {
                    cx.prompt(
                        PromptLevel::Info,
                        "Are you sure you want to quit?",
                        None,
                        &["Quit", "Cancel"],
                    )
                })
                .log_err();

            if let Some(answer) = answer {
                let answer = answer.await.ok();
                if answer != Some(0) {
                    return Ok(());
                }
            }
        }

        // If the user cancels any save prompt, then keep the app open.
        for window in workspace_windows {
            if let Some(should_close) = window
                .update(&mut cx, |workspace, cx| {
                    workspace.prepare_to_close(CloseIntent::Quit, cx)
                })
                .log_err()
            {
                if !should_close.await? {
                    return Ok(());
                }
            }
        }
        cx.update(|cx| cx.quit())?;
        anyhow::Ok(())
    })
    .detach_and_log_err(cx);
}

fn open_log_file(workspace: &mut Workspace, cx: &mut ViewContext<Workspace>) {
    const MAX_LINES: usize = 1000;
    workspace
        .with_local_workspace(cx, move |workspace, cx| {
            let fs = workspace.app_state().fs.clone();
            cx.spawn(|workspace, mut cx| async move {
                let (old_log, new_log) =
                    futures::join!(fs.load(paths::old_log_file()), fs.load(paths::log_file()));
                let log = match (old_log, new_log) {
                    (Err(_), Err(_)) => None,
                    (old_log, new_log) => {
                        let mut lines = VecDeque::with_capacity(MAX_LINES);
                        for line in old_log
                            .iter()
                            .flat_map(|log| log.lines())
                            .chain(new_log.iter().flat_map(|log| log.lines()))
                        {
                            if lines.len() == MAX_LINES {
                                lines.pop_front();
                            }
                            lines.push_back(line);
                        }
                        Some(
                            lines
                                .into_iter()
                                .flat_map(|line| [line, "\n"])
                                .collect::<String>(),
                        )
                    }
                };

                workspace
                    .update(&mut cx, |workspace, cx| {
                        let Some(log) = log else {
                            struct OpenLogError;

                            workspace.show_notification(
                                NotificationId::unique::<OpenLogError>(),
                                cx,
                                |cx| {
                                    cx.new_view(|_| {
                                        MessageNotification::new(format!(
                                            "Unable to access/open log file at path {:?}",
                                            paths::log_file().as_path()
                                        ))
                                    })
                                },
                            );
                            return;
                        };
                        let project = workspace.project().clone();
                        let buffer = project.update(cx, |project, cx| {
                            project.create_local_buffer(&log, None, cx)
                        });

                        let buffer = cx.new_model(|cx| {
                            MultiBuffer::singleton(buffer, cx).with_title("Log".into())
                        });
                        let editor = cx.new_view(|cx| {
                            let mut editor =
                                Editor::for_multibuffer(buffer, Some(project), true, cx);
                            editor.set_breadcrumb_header(format!(
                                "Last {} lines in {}",
                                MAX_LINES,
                                paths::log_file().display()
                            ));
                            editor
                        });

                        editor.update(cx, |editor, cx| {
                            let last_multi_buffer_offset = editor.buffer().read(cx).len(cx);
                            editor.change_selections(Some(Autoscroll::fit()), cx, |s| {
                                s.select_ranges(Some(
                                    last_multi_buffer_offset..last_multi_buffer_offset,
                                ));
                            })
                        });

                        workspace.add_item_to_active_pane(Box::new(editor), None, true, cx);
                    })
                    .log_err();
            })
            .detach();
        })
        .detach();
}

pub fn handle_keymap_file_changes(
    mut user_keymap_file_rx: mpsc::UnboundedReceiver<String>,
    cx: &mut AppContext,
    keymap_changed: impl Fn(Option<anyhow::Error>, &mut AppContext) + 'static,
) {
    BaseKeymap::register(cx);
    // Commented out Vim mode registration
    // VimModeSetting::register(cx);

    let (base_keymap_tx, mut base_keymap_rx) = mpsc::unbounded();
    let (keyboard_layout_tx, mut keyboard_layout_rx) = mpsc::unbounded();
    let mut old_base_keymap = *BaseKeymap::get_global(cx);
    // Commented out Vim mode state tracking
    // let mut old_vim_enabled = VimModeSetting::get_global(cx).0;
    cx.observe_global::<SettingsStore>(move |cx| {
        let new_base_keymap = *BaseKeymap::get_global(cx);
        // Commented out Vim mode state check
        // let new_vim_enabled = VimModeSetting::get_global(cx).0;

        // Modify condition to remove Vim mode comparison
        if new_base_keymap != old_base_keymap 
            // || new_vim_enabled != old_vim_enabled 
        {
            old_base_keymap = new_base_keymap;
            // Commented out Vim mode state update
            // old_vim_enabled = new_vim_enabled;
            base_keymap_tx.unbounded_send(()).unwrap();
        }
    })
    .detach();

    let mut current_mapping = settings::get_key_equivalents(cx.keyboard_layout());
    cx.on_keyboard_layout_change(move |cx| {
        let next_mapping = settings::get_key_equivalents(cx.keyboard_layout());
        if next_mapping != current_mapping {
            current_mapping = next_mapping;
            keyboard_layout_tx.unbounded_send(()).ok();
        }
    })
    .detach();

    load_default_keymap(cx);

    cx.spawn(move |cx| async move {
        let mut user_keymap = KeymapFile::default();
        loop {
            select_biased! {
                _ = base_keymap_rx.next() => {}
                _ = keyboard_layout_rx.next() => {}
                user_keymap_content = user_keymap_file_rx.next() => {
                    if let Some(user_keymap_content) = user_keymap_content {
                        match KeymapFile::parse(&user_keymap_content) {
                            Ok(keymap_content) => {
                                cx.update(|cx| keymap_changed(None, cx)).log_err();
                                user_keymap = keymap_content;
                            }
                            Err(error) => {
                                cx.update(|cx| keymap_changed(Some(error), cx)).log_err();
                            }
                        }
                    }
                }
            }
            cx.update(|cx| reload_keymaps(cx, &user_keymap)).ok();
        }
    })
    .detach();
}

fn reload_keymaps(cx: &mut AppContext, keymap_content: &KeymapFile) {
    cx.clear_key_bindings();
    load_default_keymap(cx);
    keymap_content.clone().add_to_cx(cx).log_err();
    cx.set_menus(app_menus());
    cx.set_dock_menu(vec![MenuItem::action("New Window", workspace::NewWindow)]);
}

pub fn load_default_keymap(cx: &mut AppContext) {
    let base_keymap = *BaseKeymap::get_global(cx);
    if base_keymap == BaseKeymap::None {
        return;
    }

    KeymapFile::load_asset(DEFAULT_KEYMAP_PATH, cx).unwrap();
    /*if VimModeSetting::get_global(cx).0 {
        KeymapFile::load_asset("keymaps/vim.json", cx).unwrap();
    }*/

    if let Some(asset_path) = base_keymap.asset_path() {
        KeymapFile::load_asset(asset_path, cx).unwrap();
    }
}

pub fn open_new_ssh_project_from_project(
    workspace: &mut Workspace,
    paths: Vec<PathBuf>,
    cx: &mut ViewContext<Workspace>,
) -> Task<anyhow::Result<()>> {
    let app_state = workspace.app_state().clone();
    let Some(ssh_client) = workspace.project().read(cx).ssh_client() else {
        return Task::ready(Err(anyhow::anyhow!("Not an ssh project")));
    };
    let connection_options = ssh_client.read(cx).connection_options();
    cx.spawn(|_, mut cx| async move {
        open_ssh_project(
            connection_options,
            paths,
            app_state,
            workspace::OpenOptions {
                open_new_workspace: Some(true),
                replace_window: None,
                env: None,
            },
            &mut cx,
        )
        .await
    })
}

fn open_project_settings_file(
    workspace: &mut Workspace,
    _: &OpenProjectSettings,
    cx: &mut ViewContext<Workspace>,
) {
    open_local_file(
        workspace,
        local_settings_file_relative_path(),
        initial_project_settings_content(),
        cx,
    )
}

fn open_project_tasks_file(
    workspace: &mut Workspace,
    _: &OpenProjectTasks,
    cx: &mut ViewContext<Workspace>,
) {
    open_local_file(
        workspace,
        local_tasks_file_relative_path(),
        initial_tasks_content(),
        cx,
    )
}

fn open_local_file(
    workspace: &mut Workspace,
    settings_relative_path: &'static Path,
    initial_contents: Cow<'static, str>,
    cx: &mut ViewContext<Workspace>,
) {
    let project = workspace.project().clone();
    let worktree = project
        .read(cx)
        .visible_worktrees(cx)
        .find_map(|tree| tree.read(cx).root_entry()?.is_dir().then_some(tree));
    if let Some(worktree) = worktree {
        let tree_id = worktree.read(cx).id();
        cx.spawn(|workspace, mut cx| async move {
            if let Some(dir_path) = settings_relative_path.parent() {
                if worktree.update(&mut cx, |tree, _| tree.entry_for_path(dir_path).is_none())? {
                    project
                        .update(&mut cx, |project, cx| {
                            project.create_entry((tree_id, dir_path), true, cx)
                        })?
                        .await
                        .context("worktree was removed")?;
                }
            }

            if worktree.update(&mut cx, |tree, _| {
                tree.entry_for_path(settings_relative_path).is_none()
            })? {
                project
                    .update(&mut cx, |project, cx| {
                        project.create_entry((tree_id, settings_relative_path), false, cx)
                    })?
                    .await
                    .context("worktree was removed")?;
            }

            let editor = workspace
                .update(&mut cx, |workspace, cx| {
                    workspace.open_path((tree_id, settings_relative_path), None, true, cx)
                })?
                .await?
                .downcast::<Editor>()
                .context("unexpected item type: expected editor item")?;

            editor
                .downgrade()
                .update(&mut cx, |editor, cx| {
                    if let Some(buffer) = editor.buffer().read(cx).as_singleton() {
                        if buffer.read(cx).is_empty() {
                            buffer.update(cx, |buffer, cx| {
                                buffer.edit([(0..0, initial_contents)], None, cx)
                            });
                        }
                    }
                })
                .ok();

            anyhow::Ok(())
        })
        .detach();
    } else {
        struct NoOpenFolders;

        workspace.show_notification(NotificationId::unique::<NoOpenFolders>(), cx, |cx| {
            cx.new_view(|_| MessageNotification::new("This project has no folders open."))
        })
    }
}

fn open_telemetry_log_file(workspace: &mut Workspace, cx: &mut ViewContext<Workspace>) {
    workspace.with_local_workspace(cx, move |workspace, cx| {
        let app_state = workspace.app_state().clone();
        cx.spawn(|workspace, mut cx| async move {
            async fn fetch_log_string(app_state: &Arc<AppState>) -> Option<String> {
                let path = client::telemetry::Telemetry::log_file_path();
                app_state.fs.load(&path).await.log_err()
            }

            let log = fetch_log_string(&app_state).await.unwrap_or_else(|| "// No data has been collected yet".to_string());

            const MAX_TELEMETRY_LOG_LEN: usize = 5 * 1024 * 1024;
            let mut start_offset = log.len().saturating_sub(MAX_TELEMETRY_LOG_LEN);
            if let Some(newline_offset) = log[start_offset..].find('\n') {
                start_offset += newline_offset + 1;
            }
            let log_suffix = &log[start_offset..];
            let header = concat!(
                "// Zed collects anonymous usage data to help us understand how people are using the app.\n",
                "// Telemetry can be disabled via the `settings.json` file.\n",
                "// Here is the data that has been reported for the current session:\n",
            );
            let content = format!("{}\n{}", header, log_suffix);
            let json = app_state.languages.language_for_name("JSON").await.log_err();

            workspace.update(&mut cx, |workspace, cx| {
                let project = workspace.project().clone();
                let buffer = project.update(cx, |project, cx| project.create_local_buffer(&content, json, cx));
                let buffer = cx.new_model(|cx| {
                    MultiBuffer::singleton(buffer, cx).with_title("Telemetry Log".into())
                });
                workspace.add_item_to_active_pane(
                    Box::new(cx.new_view(|cx| {
                        let mut editor = Editor::for_multibuffer(buffer, Some(project), true, cx);
                        editor.set_breadcrumb_header("Telemetry Log".into());
                        editor
                    })),
                    None,
                    true,
                    cx,
                );
            }).log_err()?;

            Some(())
        })
        .detach();
    }).detach();
}

fn open_bundled_file(
    workspace: &Workspace,
    text: Cow<'static, str>,
    title: &'static str,
    language: &'static str,
    cx: &mut ViewContext<Workspace>,
) {
    let language = workspace.app_state().languages.language_for_name(language);
    cx.spawn(|workspace, mut cx| async move {
        let language = language.await.log_err();
        workspace
            .update(&mut cx, |workspace, cx| {
                workspace.with_local_workspace(cx, |workspace, cx| {
                    let project = workspace.project();
                    let buffer = project.update(cx, move |project, cx| {
                        project.create_local_buffer(text.as_ref(), language, cx)
                    });
                    let buffer = cx.new_model(|cx| {
                        MultiBuffer::singleton(buffer, cx).with_title(title.into())
                    });
                    workspace.add_item_to_active_pane(
                        Box::new(cx.new_view(|cx| {
                            let mut editor =
                                Editor::for_multibuffer(buffer, Some(project.clone()), true, cx);
                            editor.set_read_only(true);
                            editor.set_breadcrumb_header(title.into());
                            editor
                        })),
                        None,
                        true,
                        cx,
                    );
                })
            })?
            .await
    })
    .detach_and_log_err(cx);
}

fn open_settings_file(
    abs_path: &'static Path,
    default_content: impl FnOnce() -> Rope + Send + 'static,
    cx: &mut ViewContext<Workspace>,
) {
    cx.spawn(|workspace, mut cx| async move {
        let (worktree_creation_task, settings_open_task) = workspace
            .update(&mut cx, |workspace, cx| {
                workspace.with_local_workspace(cx, move |workspace, cx| {
                    let worktree_creation_task = workspace.project().update(cx, |project, cx| {
                        // Set up a dedicated worktree for settings, since
                        // otherwise we're dropping and re-starting LSP servers
                        // for each file inside on every settings file
                        // close/open

                        // TODO: Do note that all other external files (e.g.
                        // drag and drop from OS) still have their worktrees
                        // released on file close, causing LSP servers'
                        // restarts.
                        project.find_or_create_worktree(paths::config_dir().as_path(), false, cx)
                    });
                    let settings_open_task =
                        create_and_open_local_file(abs_path, cx, default_content);
                    (worktree_creation_task, settings_open_task)
                })
            })?
            .await?;
        let _ = worktree_creation_task.await?;
        let _ = settings_open_task.await?;
        anyhow::Ok(())
    })
    .detach_and_log_err(cx);
}

async fn register_zed_scheme(cx: &AsyncAppContext) -> anyhow::Result<()> {
    cx.update(|cx| cx.register_url_scheme(ZED_URL_SCHEME))?
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use assets::Assets;
    use collections::HashSet;
    use editor::{display_map::DisplayRow, scroll::Autoscroll, DisplayPoint, Editor};
    use gpui::{
        actions, Action, AnyWindowHandle, AppContext, AssetSource, BorrowAppContext, Entity,
        SemanticVersion, TestAppContext, UpdateGlobal, VisualTestContext, WindowHandle,
    };
    use language::{LanguageMatcher, LanguageRegistry};
    use project::{project_settings::ProjectSettings, Project, ProjectPath, WorktreeSettings};
    use serde_json::json;
    use settings::{handle_settings_file_changes, watch_config_file, SettingsStore};
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };
    use theme::{ThemeRegistry, ThemeSettings};
    use workspace::{
        item::{Item, ItemHandle},
        open_new, open_paths, pane, NewFile, OpenVisible, SaveIntent, SplitDirection,
        WorkspaceHandle,
    };

    #[gpui::test]
    async fn test_open_non_existing_file(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        app_state
            .fs
            .as_fake()
            .insert_tree(
                "/root",
                json!({
                    "a": {
                    },
                }),
            )
            .await;

        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/a/new")],
                app_state.clone(),
                workspace::OpenOptions::default(),
                cx,
            )
        })
        .await
        .unwrap();
        assert_eq!(cx.read(|cx| cx.windows().len()), 1);

        let workspace = cx.windows()[0].downcast::<Workspace>().unwrap();
        workspace
            .update(cx, |workspace, cx| {
                assert!(workspace.active_item_as::<Editor>(cx).is_some())
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_open_paths_action(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        app_state
            .fs
            .as_fake()
            .insert_tree(
                "/root",
                json!({
                    "a": {
                        "aa": null,
                        "ab": null,
                    },
                    "b": {
                        "ba": null,
                        "bb": null,
                    },
                    "c": {
                        "ca": null,
                        "cb": null,
                    },
                    "d": {
                        "da": null,
                        "db": null,
                    },
                    "e": {
                        "ea": null,
                        "eb": null,
                    }
                }),
            )
            .await;

        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/a"), PathBuf::from("/root/b")],
                app_state.clone(),
                workspace::OpenOptions::default(),
                cx,
            )
        })
        .await
        .unwrap();
        assert_eq!(cx.read(|cx| cx.windows().len()), 1);

        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/a")],
                app_state.clone(),
                workspace::OpenOptions::default(),
                cx,
            )
        })
        .await
        .unwrap();
        assert_eq!(cx.read(|cx| cx.windows().len()), 1);
        let workspace_1 = cx
            .read(|cx| cx.windows()[0].downcast::<Workspace>())
            .unwrap();
        cx.run_until_parked();
        workspace_1
            .update(cx, |workspace, cx| {
                assert_eq!(workspace.worktrees(cx).count(), 2);
                assert!(workspace.left_dock().read(cx).is_open());
                assert!(workspace
                    .active_pane()
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(cx));
            })
            .unwrap();

        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/c"), PathBuf::from("/root/d")],
                app_state.clone(),
                workspace::OpenOptions::default(),
                cx,
            )
        })
        .await
        .unwrap();
        assert_eq!(cx.read(|cx| cx.windows().len()), 2);

        // Replace existing windows
        let window = cx
            .update(|cx| cx.windows()[0].downcast::<Workspace>())
            .unwrap();
        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/e")],
                app_state,
                workspace::OpenOptions {
                    replace_window: Some(window),
                    ..Default::default()
                },
                cx,
            )
        })
        .await
        .unwrap();
        cx.background_executor.run_until_parked();
        assert_eq!(cx.read(|cx| cx.windows().len()), 2);
        let workspace_1 = cx
            .update(|cx| cx.windows()[0].downcast::<Workspace>())
            .unwrap();
        workspace_1
            .update(cx, |workspace, cx| {
                assert_eq!(
                    workspace
                        .worktrees(cx)
                        .map(|w| w.read(cx).abs_path())
                        .collect::<Vec<_>>(),
                    &[Path::new("/root/e").into()]
                );
                assert!(workspace.left_dock().read(cx).is_open());
                assert!(workspace.active_pane().focus_handle(cx).is_focused(cx));
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_open_add_new(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        app_state
            .fs
            .as_fake()
            .insert_tree("/root", json!({"a": "hey", "b": "", "dir": {"c": "f"}}))
            .await;

        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/dir")],
                app_state.clone(),
                workspace::OpenOptions::default(),
                cx,
            )
        })
        .await
        .unwrap();
        assert_eq!(cx.update(|cx| cx.windows().len()), 1);

        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/a")],
                app_state.clone(),
                workspace::OpenOptions {
                    open_new_workspace: Some(false),
                    ..Default::default()
                },
                cx,
            )
        })
        .await
        .unwrap();
        assert_eq!(cx.update(|cx| cx.windows().len()), 1);

        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/dir/c")],
                app_state.clone(),
                workspace::OpenOptions {
                    open_new_workspace: Some(true),
                    ..Default::default()
                },
                cx,
            )
        })
        .await
        .unwrap();
        assert_eq!(cx.update(|cx| cx.windows().len()), 2);
    }

    #[gpui::test]
    async fn test_open_file_in_many_spaces(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        app_state
            .fs
            .as_fake()
            .insert_tree("/root", json!({"dir1": {"a": "b"}, "dir2": {"c": "d"}}))
            .await;

        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/dir1/a")],
                app_state.clone(),
                workspace::OpenOptions::default(),
                cx,
            )
        })
        .await
        .unwrap();
        assert_eq!(cx.update(|cx| cx.windows().len()), 1);
        let window1 = cx.update(|cx| cx.active_window().unwrap());

        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/dir2/c")],
                app_state.clone(),
                workspace::OpenOptions::default(),
                cx,
            )
        })
        .await
        .unwrap();
        assert_eq!(cx.update(|cx| cx.windows().len()), 1);

        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/dir2")],
                app_state.clone(),
                workspace::OpenOptions::default(),
                cx,
            )
        })
        .await
        .unwrap();
        assert_eq!(cx.update(|cx| cx.windows().len()), 2);
        let window2 = cx.update(|cx| cx.active_window().unwrap());
        assert!(window1 != window2);
        cx.update_window(window1, |_, cx| cx.activate_window())
            .unwrap();

        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/dir2/c")],
                app_state.clone(),
                workspace::OpenOptions::default(),
                cx,
            )
        })
        .await
        .unwrap();
        assert_eq!(cx.update(|cx| cx.windows().len()), 2);
        // should have opened in window2 because that has dir2 visibly open (window1 has it open, but not in the project panel)
        assert!(cx.update(|cx| cx.active_window().unwrap()) == window2);
    }

    #[gpui::test]
    async fn test_window_edit_state_restoring_disabled(cx: &mut TestAppContext) {
        let executor = cx.executor();
        let app_state = init_test(cx);

        cx.update(|cx| {
            SettingsStore::update_global(cx, |store, cx| {
                store.update_user_settings::<ProjectSettings>(cx, |settings| {
                    settings.session.restore_unsaved_buffers = false
                });
            });
        });

        app_state
            .fs
            .as_fake()
            .insert_tree("/root", json!({"a": "hey"}))
            .await;

        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/a")],
                app_state.clone(),
                workspace::OpenOptions::default(),
                cx,
            )
        })
        .await
        .unwrap();
        assert_eq!(cx.update(|cx| cx.windows().len()), 1);

        // When opening the workspace, the window is not in a edited state.
        let window = cx.update(|cx| cx.windows()[0].downcast::<Workspace>().unwrap());

        let window_is_edited = |window: WindowHandle<Workspace>, cx: &mut TestAppContext| {
            cx.update(|cx| window.read(cx).unwrap().is_edited())
        };
        let pane = window
            .read_with(cx, |workspace, _| workspace.active_pane().clone())
            .unwrap();
        let editor = window
            .read_with(cx, |workspace, cx| {
                workspace
                    .active_item(cx)
                    .unwrap()
                    .downcast::<Editor>()
                    .unwrap()
            })
            .unwrap();

        assert!(!window_is_edited(window, cx));

        // Editing a buffer marks the window as edited.
        window
            .update(cx, |_, cx| {
                editor.update(cx, |editor, cx| editor.insert("EDIT", cx));
            })
            .unwrap();

        assert!(window_is_edited(window, cx));

        // Undoing the edit restores the window's edited state.
        window
            .update(cx, |_, cx| {
                editor.update(cx, |editor, cx| editor.undo(&Default::default(), cx));
            })
            .unwrap();
        assert!(!window_is_edited(window, cx));

        // Redoing the edit marks the window as edited again.
        window
            .update(cx, |_, cx| {
                editor.update(cx, |editor, cx| editor.redo(&Default::default(), cx));
            })
            .unwrap();
        assert!(window_is_edited(window, cx));

        // Closing the item restores the window's edited state.
        let close = window
            .update(cx, |_, cx| {
                pane.update(cx, |pane, cx| {
                    drop(editor);
                    pane.close_active_item(&Default::default(), cx).unwrap()
                })
            })
            .unwrap();
        executor.run_until_parked();

        cx.simulate_prompt_answer(1);
        close.await.unwrap();
        assert!(!window_is_edited(window, cx));

        // Advance the clock to ensure that the item has been serialized and dropped from the queue
        cx.executor().advance_clock(Duration::from_secs(1));

        // Opening the buffer again doesn't impact the window's edited state.
        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/a")],
                app_state,
                workspace::OpenOptions::default(),
                cx,
            )
        })
        .await
        .unwrap();
        executor.run_until_parked();

        window
            .update(cx, |workspace, cx| {
                let editor = workspace
                    .active_item(cx)
                    .unwrap()
                    .downcast::<Editor>()
                    .unwrap();

                editor.update(cx, |editor, cx| {
                    assert_eq!(editor.text(cx), "hey");
                });
            })
            .unwrap();

        let editor = window
            .read_with(cx, |workspace, cx| {
                workspace
                    .active_item(cx)
                    .unwrap()
                    .downcast::<Editor>()
                    .unwrap()
            })
            .unwrap();
        assert!(!window_is_edited(window, cx));

        // Editing the buffer marks the window as edited.
        window
            .update(cx, |_, cx| {
                editor.update(cx, |editor, cx| editor.insert("EDIT", cx));
            })
            .unwrap();
        executor.run_until_parked();
        assert!(window_is_edited(window, cx));

        // Ensure closing the window via the mouse gets preempted due to the
        // buffer having unsaved changes.
        assert!(!VisualTestContext::from_window(window.into(), cx).simulate_close());
        executor.run_until_parked();
        assert_eq!(cx.update(|cx| cx.windows().len()), 1);

        // The window is successfully closed after the user dismisses the prompt.
        cx.simulate_prompt_answer(1);
        executor.run_until_parked();
        assert_eq!(cx.update(|cx| cx.windows().len()), 0);
    }

    #[gpui::test]
    async fn test_window_edit_state_restoring_enabled(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        app_state
            .fs
            .as_fake()
            .insert_tree("/root", json!({"a": "hey"}))
            .await;

        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/a")],
                app_state.clone(),
                workspace::OpenOptions::default(),
                cx,
            )
        })
        .await
        .unwrap();

        assert_eq!(cx.update(|cx| cx.windows().len()), 1);

        // When opening the workspace, the window is not in a edited state.
        let window = cx.update(|cx| cx.windows()[0].downcast::<Workspace>().unwrap());

        let window_is_edited = |window: WindowHandle<Workspace>, cx: &mut TestAppContext| {
            cx.update(|cx| window.read(cx).unwrap().is_edited())
        };

        let editor = window
            .read_with(cx, |workspace, cx| {
                workspace
                    .active_item(cx)
                    .unwrap()
                    .downcast::<Editor>()
                    .unwrap()
            })
            .unwrap();

        assert!(!window_is_edited(window, cx));

        // Editing a buffer marks the window as edited.
        window
            .update(cx, |_, cx| {
                editor.update(cx, |editor, cx| editor.insert("EDIT", cx));
            })
            .unwrap();

        assert!(window_is_edited(window, cx));
        cx.run_until_parked();

        // Advance the clock to make sure the workspace is serialized
        cx.executor().advance_clock(Duration::from_secs(1));

        // When closing the window, no prompt shows up and the window is closed.
        // buffer having unsaved changes.
        assert!(!VisualTestContext::from_window(window.into(), cx).simulate_close());
        cx.run_until_parked();
        assert_eq!(cx.update(|cx| cx.windows().len()), 0);

        // When we now reopen the window, the edited state and the edited buffer are back
        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/root/a")],
                app_state.clone(),
                workspace::OpenOptions::default(),
                cx,
            )
        })
        .await
        .unwrap();

        assert_eq!(cx.update(|cx| cx.windows().len()), 1);
        assert!(cx.update(|cx| cx.active_window().is_some()));

        // When opening the workspace, the window is not in a edited state.
        let window = cx.update(|cx| cx.active_window().unwrap().downcast::<Workspace>().unwrap());
        assert!(window_is_edited(window, cx));

        window
            .update(cx, |workspace, cx| {
                let editor = workspace
                    .active_item(cx)
                    .unwrap()
                    .downcast::<editor::Editor>()
                    .unwrap();
                editor.update(cx, |editor, cx| {
                    assert_eq!(editor.text(cx), "EDIThey");
                    assert!(editor.is_dirty(cx));
                });

                editor
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_new_empty_workspace(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        cx.update(|cx| {
            open_new(
                Default::default(),
                app_state.clone(),
                cx,
                |workspace, cx| Editor::new_file(workspace, &Default::default(), cx),
            )
        })
        .await
        .unwrap();
        cx.run_until_parked();

        let workspace = cx
            .update(|cx| cx.windows().first().unwrap().downcast::<Workspace>())
            .unwrap();

        let editor = workspace
            .update(cx, |workspace, cx| {
                let editor = workspace
                    .active_item(cx)
                    .unwrap()
                    .downcast::<editor::Editor>()
                    .unwrap();
                editor.update(cx, |editor, cx| {
                    assert!(editor.text(cx).is_empty());
                    assert!(!editor.is_dirty(cx));
                });

                editor
            })
            .unwrap();

        let save_task = workspace
            .update(cx, |workspace, cx| {
                workspace.save_active_item(SaveIntent::Save, cx)
            })
            .unwrap();
        app_state.fs.create_dir(Path::new("/root")).await.unwrap();
        cx.background_executor.run_until_parked();
        cx.simulate_new_path_selection(|_| Some(PathBuf::from("/root/the-new-name")));
        save_task.await.unwrap();
        workspace
            .update(cx, |_, cx| {
                editor.update(cx, |editor, cx| {
                    assert!(!editor.is_dirty(cx));
                    assert_eq!(editor.title(cx), "the-new-name");
                });
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_open_entry(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        app_state
            .fs
            .as_fake()
            .insert_tree(
                "/root",
                json!({
                    "a": {
                        "file1": "contents 1",
                        "file2": "contents 2",
                        "file3": "contents 3",
                    },
                }),
            )
            .await;

        let project = Project::test(app_state.fs.clone(), ["/root".as_ref()], cx).await;
        project.update(cx, |project, _cx| {
            project.languages().add(markdown_language())
        });
        let window = cx.add_window(|cx| Workspace::test_new(project, cx));
        let workspace = window.root(cx).unwrap();

        let entries = cx.read(|cx| workspace.file_project_paths(cx));
        let file1 = entries[0].clone();
        let file2 = entries[1].clone();
        let file3 = entries[2].clone();

        // Open the first entry
        let entry_1 = window
            .update(cx, |w, cx| w.open_path(file1.clone(), None, true, cx))
            .unwrap()
            .await
            .unwrap();
        cx.read(|cx| {
            let pane = workspace.read(cx).active_pane().read(cx);
            assert_eq!(
                pane.active_item().unwrap().project_path(cx),
                Some(file1.clone())
            );
            assert_eq!(pane.items_len(), 1);
        });

        // Open the second entry
        window
            .update(cx, |w, cx| w.open_path(file2.clone(), None, true, cx))
            .unwrap()
            .await
            .unwrap();
        cx.read(|cx| {
            let pane = workspace.read(cx).active_pane().read(cx);
            assert_eq!(
                pane.active_item().unwrap().project_path(cx),
                Some(file2.clone())
            );
            assert_eq!(pane.items_len(), 2);
        });

        // Open the first entry again. The existing pane item is activated.
        let entry_1b = window
            .update(cx, |w, cx| w.open_path(file1.clone(), None, true, cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(entry_1.item_id(), entry_1b.item_id());

        cx.read(|cx| {
            let pane = workspace.read(cx).active_pane().read(cx);
            assert_eq!(
                pane.active_item().unwrap().project_path(cx),
                Some(file1.clone())
            );
            assert_eq!(pane.items_len(), 2);
        });

        // Split the pane with the first entry, then open the second entry again.
        window
            .update(cx, |w, cx| {
                w.split_and_clone(w.active_pane().clone(), SplitDirection::Right, cx);
                w.open_path(file2.clone(), None, true, cx)
            })
            .unwrap()
            .await
            .unwrap();

        window
            .read_with(cx, |w, cx| {
                assert_eq!(
                    w.active_pane()
                        .read(cx)
                        .active_item()
                        .unwrap()
                        .project_path(cx),
                    Some(file2.clone())
                );
            })
            .unwrap();

        // Open the third entry twice concurrently. Only one pane item is added.
        let (t1, t2) = window
            .update(cx, |w, cx| {
                (
                    w.open_path(file3.clone(), None, true, cx),
                    w.open_path(file3.clone(), None, true, cx),
                )
            })
            .unwrap();
        t1.await.unwrap();
        t2.await.unwrap();
        cx.read(|cx| {
            let pane = workspace.read(cx).active_pane().read(cx);
            assert_eq!(
                pane.active_item().unwrap().project_path(cx),
                Some(file3.clone())
            );
            let pane_entries = pane
                .items()
                .map(|i| i.project_path(cx).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(pane_entries, &[file1, file2, file3]);
        });
    }

    #[gpui::test]
    async fn test_open_paths(cx: &mut TestAppContext) {
        let app_state = init_test(cx);

        app_state
            .fs
            .as_fake()
            .insert_tree(
                "/",
                json!({
                    "dir1": {
                        "a.txt": ""
                    },
                    "dir2": {
                        "b.txt": ""
                    },
                    "dir3": {
                        "c.txt": ""
                    },
                    "d.txt": ""
                }),
            )
            .await;

        cx.update(|cx| {
            open_paths(
                &[PathBuf::from("/dir1/")],
                app_state,
                workspace::OpenOptions::default(),
                cx,
            )
        })
        .await
        .unwrap();
        assert_eq!(cx.update(|cx| cx.windows().len()), 1);
        let window = cx.update(|cx| cx.windows()[0].downcast::<Workspace>().unwrap());
        let workspace = window.root(cx).unwrap();

        #[track_caller]
        fn assert_project_panel_selection(
            workspace: &Workspace,
            expected_worktree_path: &Path,
            expected_entry_path: &Path,
            cx: &AppContext,
        ) {
            let project_panel = [
                workspace.left_dock().read(cx).panel::<ProjectPanel>(),
                workspace.right_dock().read(cx).panel::<ProjectPanel>(),
                workspace.bottom_dock().read(cx).panel::<ProjectPanel>(),
            ]
            .into_iter()
            .find_map(std::convert::identity)
            .expect("found no project panels")
            .read(cx);
            let (selected_worktree, selected_entry) = project_panel
                .selected_entry(cx)
                .expect("project panel should have a selected entry");
            assert_eq!(
                selected_worktree.abs_path().as_ref(),
                expected_worktree_path,
                "Unexpected project panel selected worktree path"
            );
            assert_eq!(
                selected_entry.path.as_ref(),
                expected_entry_path,
                "Unexpected project panel selected entry path"
            );
        }

        // Open a file within an existing worktree.
        window
            .update(cx, |view, cx| {
                view.open_paths(vec!["/dir1/a.txt".into()], OpenVisible::All, None, cx)
            })
            .unwrap()
            .await;
        cx.read(|cx| {
            let workspace = workspace.read(cx);
            assert_project_panel_selection(workspace, Path::new("/dir1"), Path::new("a.txt"), cx);
            assert_eq!(
                workspace
                    .active_pane()
                    .read(cx)
                    .active_item()
                    .unwrap()
                    .act_as::<Editor>(cx)
                    .unwrap()
                    .read(cx)
                    .title(cx),
                "a.txt"
            );
        });

        // Open a file outside of any existing worktree.
        window
            .update(cx, |view, cx| {
                view.open_paths(vec!["/dir2/b.txt".into()], OpenVisible::All, None, cx)
            })
            .unwrap()
            .await;
        cx.read(|cx| {
            let workspace = workspace.read(cx);
            assert_project_panel_selection(workspace, Path::new("/dir2/b.txt"), Path::new(""), cx);
            let worktree_roots = workspace
                .worktrees(cx)
                .map(|w| w.read(cx).as_local().unwrap().abs_path().as_ref())
                .collect::<HashSet<_>>();
            assert_eq!(
                worktree_roots,
                vec!["/dir1", "/dir2/b.txt"]
                    .into_iter()
                    .map(Path::new)
                    .collect(),
            );
            assert_eq!(
                workspace
                    .active_pane()
                    .read(cx)
                    .active_item()
                    .unwrap()
                    .act_as::<Editor>(cx)
                    .unwrap()
                    .read(cx)
                    .title(cx),
                "b.txt"
            );
        });

        // Ensure opening a directory and one of its children only adds one worktree.
        window
            .update(cx, |view, cx| {
                view.open_paths(
                    vec!["/dir3".into(), "/dir3/c.txt".into()],
                    OpenVisible::All,
                    None,
                    cx,
                )
            })
            .unwrap()
            .await;
        cx.read(|cx| {
            let workspace = workspace.read(cx);
            assert_project_panel_selection(workspace, Path::new("/dir3"), Path::new("c.txt"), cx);
            let worktree_roots = workspace
                .worktrees(cx)
                .map(|w| w.read(cx).as_local().unwrap().abs_path().as_ref())
                .collect::<HashSet<_>>();
            assert_eq!(
                worktree_roots,
                vec!["/dir1", "/dir2/b.txt", "/dir3"]
                    .into_iter()
                    .map(Path::new)
                    .collect(),
            );
            assert_eq!(
                workspace
                    .active_pane()
                    .read(cx)
                    .active_item()
                    .unwrap()
                    .act_as::<Editor>(cx)
                    .unwrap()
                    .read(cx)
                    .title(cx),
                "c.txt"
            );
        });

        // Ensure opening invisibly a file outside an existing worktree adds a new, invisible worktree.
        window
            .update(cx, |view, cx| {
                view.open_paths(vec!["/d.txt".into()], OpenVisible::None, None, cx)
            })
            .unwrap()
            .await;
        cx.read(|cx| {
            let workspace = workspace.read(cx);
            assert_project_panel_selection(workspace, Path::new("/d.txt"), Path::new(""), cx);
            let worktree_roots = workspace
                .worktrees(cx)
                .map(|w| w.read(cx).as_local().unwrap().abs_path().as_ref())
                .collect::<HashSet<_>>();
            assert_eq!(
                worktree_roots,
                vec!["/dir1", "/dir2/b.txt", "/dir3", "/d.txt"]
                    .into_iter()
                    .map(Path::new)
                    .collect(),
            );

            let visible_worktree_roots = workspace
                .visible_worktrees(cx)
                .map(|w| w.read(cx).as_local().unwrap().abs_path().as_ref())
                .collect::<HashSet<_>>();
            assert_eq!(
                visible_worktree_roots,
                vec!["/dir1", "/dir2/b.txt", "/dir3"]
                    .into_iter()
                    .map(Path::new)
                    .collect(),
            );

            assert_eq!(
                workspace
                    .active_pane()
                    .read(cx)
                    .active_item()
                    .unwrap()
                    .act_as::<Editor>(cx)
                    .unwrap()
                    .read(cx)
                    .title(cx),
                "d.txt"
            );
        });
    }

    #[gpui::test]
    async fn test_opening_excluded_paths(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        cx.update(|cx| {
            cx.update_global::<SettingsStore, _>(|store, cx| {
                store.update_user_settings::<WorktreeSettings>(cx, |project_settings| {
                    project_settings.file_scan_exclusions =
                        Some(vec!["excluded_dir".to_string(), "**/.git".to_string()]);
                });
            });
        });
        app_state
            .fs
            .as_fake()
            .insert_tree(
                "/root",
                json!({
                    ".gitignore": "ignored_dir\n",
                    ".git": {
                        "HEAD": "ref: refs/heads/main",
                    },
                    "regular_dir": {
                        "file": "regular file contents",
                    },
                    "ignored_dir": {
                        "ignored_subdir": {
                            "file": "ignored subfile contents",
                        },
                        "file": "ignored file contents",
                    },
                    "excluded_dir": {
                        "file": "excluded file contents",
                        "ignored_subdir": {
                            "file": "ignored subfile contents",
                        },
                    },
                }),
            )
            .await;

        let project = Project::test(app_state.fs.clone(), ["/root".as_ref()], cx).await;
        project.update(cx, |project, _cx| {
            project.languages().add(markdown_language())
        });
        let window = cx.add_window(|cx| Workspace::test_new(project, cx));
        let workspace = window.root(cx).unwrap();

        let initial_entries = cx.read(|cx| workspace.file_project_paths(cx));
        let paths_to_open = [
            Path::new("/root/excluded_dir/file").to_path_buf(),
            Path::new("/root/.git/HEAD").to_path_buf(),
            Path::new("/root/excluded_dir/ignored_subdir").to_path_buf(),
        ];
        let (opened_workspace, new_items) = cx
            .update(|cx| {
                workspace::open_paths(
                    &paths_to_open,
                    app_state,
                    workspace::OpenOptions::default(),
                    cx,
                )
            })
            .await
            .unwrap();

        assert_eq!(
            opened_workspace.root_view(cx).unwrap().entity_id(),
            workspace.entity_id(),
            "Excluded files in subfolders of a workspace root should be opened in the workspace"
        );
        let mut opened_paths = cx.read(|cx| {
            assert_eq!(
                new_items.len(),
                paths_to_open.len(),
                "Expect to get the same number of opened items as submitted paths to open"
            );
            new_items
                .iter()
                .zip(paths_to_open.iter())
                .map(|(i, path)| {
                    match i {
                        Some(Ok(i)) => {
                            Some(i.project_path(cx).map(|p| p.path.display().to_string()))
                        }
                        Some(Err(e)) => panic!("Excluded file {path:?} failed to open: {e:?}"),
                        None => None,
                    }
                    .flatten()
                })
                .collect::<Vec<_>>()
        });
        opened_paths.sort();
        assert_eq!(
            opened_paths,
            vec![
                None,
                Some(".git/HEAD".to_string()),
                Some("excluded_dir/file".to_string()),
            ],
            "Excluded files should get opened, excluded dir should not get opened"
        );

        let entries = cx.read(|cx| workspace.file_project_paths(cx));
        assert_eq!(
                initial_entries, entries,
                "Workspace entries should not change after opening excluded files and directories paths"
            );

        cx.read(|cx| {
                let pane = workspace.read(cx).active_pane().read(cx);
                let mut opened_buffer_paths = pane
                    .items()
                    .map(|i| {
                        i.project_path(cx)
                            .expect("all excluded files that got open should have a path")
                            .path
                            .display()
                            .to_string()
                    })
                    .collect::<Vec<_>>();
                opened_buffer_paths.sort();
                assert_eq!(
                    opened_buffer_paths,
                    vec![".git/HEAD".to_string(), "excluded_dir/file".to_string()],
                    "Despite not being present in the worktrees, buffers for excluded files are opened and added to the pane"
                );
            });
    }

    #[gpui::test]
    async fn test_save_conflicting_item(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        app_state
            .fs
            .as_fake()
            .insert_tree("/root", json!({ "a.txt": "" }))
            .await;

        let project = Project::test(app_state.fs.clone(), ["/root".as_ref()], cx).await;
        project.update(cx, |project, _cx| {
            project.languages().add(markdown_language())
        });
        let window = cx.add_window(|cx| Workspace::test_new(project, cx));
        let workspace = window.root(cx).unwrap();

        // Open a file within an existing worktree.
        window
            .update(cx, |view, cx| {
                view.open_paths(
                    vec![PathBuf::from("/root/a.txt")],
                    OpenVisible::All,
                    None,
                    cx,
                )
            })
            .unwrap()
            .await;
        let editor = cx.read(|cx| {
            let pane = workspace.read(cx).active_pane().read(cx);
            let item = pane.active_item().unwrap();
            item.downcast::<Editor>().unwrap()
        });

        window
            .update(cx, |_, cx| {
                editor.update(cx, |editor, cx| editor.handle_input("x", cx));
            })
            .unwrap();

        app_state
            .fs
            .as_fake()
            .insert_file("/root/a.txt", b"changed".to_vec())
            .await;

        cx.run_until_parked();
        cx.read(|cx| assert!(editor.is_dirty(cx)));
        cx.read(|cx| assert!(editor.has_conflict(cx)));

        let save_task = window
            .update(cx, |workspace, cx| {
                workspace.save_active_item(SaveIntent::Save, cx)
            })
            .unwrap();
        cx.background_executor.run_until_parked();
        cx.simulate_prompt_answer(0);
        save_task.await.unwrap();
        window
            .update(cx, |_, cx| {
                editor.update(cx, |editor, cx| {
                    assert!(!editor.is_dirty(cx));
                    assert!(!editor.has_conflict(cx));
                });
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_open_and_save_new_file(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        app_state.fs.create_dir(Path::new("/root")).await.unwrap();

        let project = Project::test(app_state.fs.clone(), ["/root".as_ref()], cx).await;
        project.update(cx, |project, _| {
            project.languages().add(markdown_language());
            project.languages().add(rust_lang());
        });
        let window = cx.add_window(|cx| Workspace::test_new(project, cx));
        let worktree = cx.update(|cx| window.read(cx).unwrap().worktrees(cx).next().unwrap());

        // Create a new untitled buffer
        cx.dispatch_action(window.into(), NewFile);
        let editor = window
            .read_with(cx, |workspace, cx| {
                workspace
                    .active_item(cx)
                    .unwrap()
                    .downcast::<Editor>()
                    .unwrap()
            })
            .unwrap();

        window
            .update(cx, |_, cx| {
                editor.update(cx, |editor, cx| {
                    assert!(!editor.is_dirty(cx));
                    assert_eq!(editor.title(cx), "untitled");
                    assert!(Arc::ptr_eq(
                        &editor.buffer().read(cx).language_at(0, cx).unwrap(),
                        &languages::PLAIN_TEXT
                    ));
                    editor.handle_input("hi", cx);
                    assert!(editor.is_dirty(cx));
                });
            })
            .unwrap();

        // Save the buffer. This prompts for a filename.
        let save_task = window
            .update(cx, |workspace, cx| {
                workspace.save_active_item(SaveIntent::Save, cx)
            })
            .unwrap();
        cx.background_executor.run_until_parked();
        cx.simulate_new_path_selection(|parent_dir| {
            assert_eq!(parent_dir, Path::new("/root"));
            Some(parent_dir.join("the-new-name.rs"))
        });
        cx.read(|cx| {
            assert!(editor.is_dirty(cx));
            assert_eq!(editor.read(cx).title(cx), "untitled");
        });

        // When the save completes, the buffer's title is updated and the language is assigned based
        // on the path.
        save_task.await.unwrap();
        window
            .update(cx, |_, cx| {
                editor.update(cx, |editor, cx| {
                    assert!(!editor.is_dirty(cx));
                    assert_eq!(editor.title(cx), "the-new-name.rs");
                    assert_eq!(
                        editor.buffer().read(cx).language_at(0, cx).unwrap().name(),
                        "Rust".into()
                    );
                });
            })
            .unwrap();

        // Edit the file and save it again. This time, there is no filename prompt.
        window
            .update(cx, |_, cx| {
                editor.update(cx, |editor, cx| {
                    editor.handle_input(" there", cx);
                    assert!(editor.is_dirty(cx));
                });
            })
            .unwrap();

        let save_task = window
            .update(cx, |workspace, cx| {
                workspace.save_active_item(SaveIntent::Save, cx)
            })
            .unwrap();
        save_task.await.unwrap();

        assert!(!cx.did_prompt_for_new_path());
        window
            .update(cx, |_, cx| {
                editor.update(cx, |editor, cx| {
                    assert!(!editor.is_dirty(cx));
                    assert_eq!(editor.title(cx), "the-new-name.rs")
                });
            })
            .unwrap();

        // Open the same newly-created file in another pane item. The new editor should reuse
        // the same buffer.
        cx.dispatch_action(window.into(), NewFile);
        window
            .update(cx, |workspace, cx| {
                workspace.split_and_clone(
                    workspace.active_pane().clone(),
                    SplitDirection::Right,
                    cx,
                );
                workspace.open_path((worktree.read(cx).id(), "the-new-name.rs"), None, true, cx)
            })
            .unwrap()
            .await
            .unwrap();
        let editor2 = window
            .update(cx, |workspace, cx| {
                workspace
                    .active_item(cx)
                    .unwrap()
                    .downcast::<Editor>()
                    .unwrap()
            })
            .unwrap();
        cx.read(|cx| {
            assert_eq!(
                editor2.read(cx).buffer().read(cx).as_singleton().unwrap(),
                editor.read(cx).buffer().read(cx).as_singleton().unwrap()
            );
        })
    }

    #[gpui::test]
    async fn test_setting_language_when_saving_as_single_file_worktree(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        app_state.fs.create_dir(Path::new("/root")).await.unwrap();

        let project = Project::test(app_state.fs.clone(), [], cx).await;
        project.update(cx, |project, _| {
            project.languages().add(rust_lang());
            project.languages().add(markdown_language());
        });
        let window = cx.add_window(|cx| Workspace::test_new(project, cx));

        // Create a new untitled buffer
        cx.dispatch_action(window.into(), NewFile);
        let editor = window
            .read_with(cx, |workspace, cx| {
                workspace
                    .active_item(cx)
                    .unwrap()
                    .downcast::<Editor>()
                    .unwrap()
            })
            .unwrap();
        window
            .update(cx, |_, cx| {
                editor.update(cx, |editor, cx| {
                    assert!(Arc::ptr_eq(
                        &editor.buffer().read(cx).language_at(0, cx).unwrap(),
                        &languages::PLAIN_TEXT
                    ));
                    editor.handle_input("hi", cx);
                    assert!(editor.is_dirty(cx));
                });
            })
            .unwrap();

        // Save the buffer. This prompts for a filename.
        let save_task = window
            .update(cx, |workspace, cx| {
                workspace.save_active_item(SaveIntent::Save, cx)
            })
            .unwrap();
        cx.background_executor.run_until_parked();
        cx.simulate_new_path_selection(|_| Some(PathBuf::from("/root/the-new-name.rs")));
        save_task.await.unwrap();
        // The buffer is not dirty anymore and the language is assigned based on the path.
        window
            .update(cx, |_, cx| {
                editor.update(cx, |editor, cx| {
                    assert!(!editor.is_dirty(cx));
                    assert_eq!(
                        editor.buffer().read(cx).language_at(0, cx).unwrap().name(),
                        "Rust".into()
                    )
                });
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_pane_actions(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        app_state
            .fs
            .as_fake()
            .insert_tree(
                "/root",
                json!({
                    "a": {
                        "file1": "contents 1",
                        "file2": "contents 2",
                        "file3": "contents 3",
                    },
                }),
            )
            .await;

        let project = Project::test(app_state.fs.clone(), ["/root".as_ref()], cx).await;
        project.update(cx, |project, _cx| {
            project.languages().add(markdown_language())
        });
        let window = cx.add_window(|cx| Workspace::test_new(project, cx));
        let workspace = window.root(cx).unwrap();

        let entries = cx.read(|cx| workspace.file_project_paths(cx));
        let file1 = entries[0].clone();

        let pane_1 = cx.read(|cx| workspace.read(cx).active_pane().clone());

        window
            .update(cx, |w, cx| w.open_path(file1.clone(), None, true, cx))
            .unwrap()
            .await
            .unwrap();

        let (editor_1, buffer) = window
            .update(cx, |_, cx| {
                pane_1.update(cx, |pane_1, cx| {
                    let editor = pane_1.active_item().unwrap().downcast::<Editor>().unwrap();
                    assert_eq!(editor.project_path(cx), Some(file1.clone()));
                    let buffer = editor.update(cx, |editor, cx| {
                        editor.insert("dirt", cx);
                        editor.buffer().downgrade()
                    });
                    (editor.downgrade(), buffer)
                })
            })
            .unwrap();

        cx.dispatch_action(window.into(), pane::SplitRight);
        let editor_2 = cx.update(|cx| {
            let pane_2 = workspace.read(cx).active_pane().clone();
            assert_ne!(pane_1, pane_2);

            let pane2_item = pane_2.read(cx).active_item().unwrap();
            assert_eq!(pane2_item.project_path(cx), Some(file1.clone()));

            pane2_item.downcast::<Editor>().unwrap().downgrade()
        });
        cx.dispatch_action(
            window.into(),
            workspace::CloseActiveItem { save_intent: None },
        );

        cx.background_executor.run_until_parked();
        window
            .read_with(cx, |workspace, _| {
                assert_eq!(workspace.panes().len(), 1);
                assert_eq!(workspace.active_pane(), &pane_1);
            })
            .unwrap();

        cx.dispatch_action(
            window.into(),
            workspace::CloseActiveItem { save_intent: None },
        );
        cx.background_executor.run_until_parked();
        cx.simulate_prompt_answer(1);
        cx.background_executor.run_until_parked();

        window
            .update(cx, |workspace, cx| {
                assert_eq!(workspace.panes().len(), 1);
                assert!(workspace.active_item(cx).is_none());
            })
            .unwrap();

        cx.run_until_parked();
        editor_1.assert_released();
        editor_2.assert_released();
        buffer.assert_released();
    }

    #[gpui::test]
    async fn test_navigation(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        app_state
            .fs
            .as_fake()
            .insert_tree(
                "/root",
                json!({
                    "a": {
                        "file1": "contents 1\n".repeat(20),
                        "file2": "contents 2\n".repeat(20),
                        "file3": "contents 3\n".repeat(20),
                    },
                }),
            )
            .await;

        let project = Project::test(app_state.fs.clone(), ["/root".as_ref()], cx).await;
        project.update(cx, |project, _cx| {
            project.languages().add(markdown_language())
        });
        let workspace = cx.add_window(|cx| Workspace::test_new(project.clone(), cx));
        let pane = workspace
            .read_with(cx, |workspace, _| workspace.active_pane().clone())
            .unwrap();

        let entries = cx.update(|cx| workspace.root(cx).unwrap().file_project_paths(cx));
        let file1 = entries[0].clone();
        let file2 = entries[1].clone();
        let file3 = entries[2].clone();

        let editor1 = workspace
            .update(cx, |w, cx| w.open_path(file1.clone(), None, true, cx))
            .unwrap()
            .await
            .unwrap()
            .downcast::<Editor>()
            .unwrap();
        workspace
            .update(cx, |_, cx| {
                editor1.update(cx, |editor, cx| {
                    editor.change_selections(Some(Autoscroll::fit()), cx, |s| {
                        s.select_display_ranges([DisplayPoint::new(DisplayRow(10), 0)
                            ..DisplayPoint::new(DisplayRow(10), 0)])
                    });
                });
            })
            .unwrap();

        let editor2 = workspace
            .update(cx, |w, cx| w.open_path(file2.clone(), None, true, cx))
            .unwrap()
            .await
            .unwrap()
            .downcast::<Editor>()
            .unwrap();
        let editor3 = workspace
            .update(cx, |w, cx| w.open_path(file3.clone(), None, true, cx))
            .unwrap()
            .await
            .unwrap()
            .downcast::<Editor>()
            .unwrap();

        workspace
            .update(cx, |_, cx| {
                editor3.update(cx, |editor, cx| {
                    editor.change_selections(Some(Autoscroll::fit()), cx, |s| {
                        s.select_display_ranges([DisplayPoint::new(DisplayRow(12), 0)
                            ..DisplayPoint::new(DisplayRow(12), 0)])
                    });
                    editor.newline(&Default::default(), cx);
                    editor.newline(&Default::default(), cx);
                    editor.move_down(&Default::default(), cx);
                    editor.move_down(&Default::default(), cx);
                    editor.save(true, project.clone(), cx)
                })
            })
            .unwrap()
            .await
            .unwrap();
        workspace
            .update(cx, |_, cx| {
                editor3.update(cx, |editor, cx| {
                    editor.set_scroll_position(point(0., 12.5), cx)
                });
            })
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file3.clone(), DisplayPoint::new(DisplayRow(16), 0), 12.5)
        );

        workspace
            .update(cx, |w, cx| w.go_back(w.active_pane().downgrade(), cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file3.clone(), DisplayPoint::new(DisplayRow(0), 0), 0.)
        );

        workspace
            .update(cx, |w, cx| w.go_back(w.active_pane().downgrade(), cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file2.clone(), DisplayPoint::new(DisplayRow(0), 0), 0.)
        );

        workspace
            .update(cx, |w, cx| w.go_back(w.active_pane().downgrade(), cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file1.clone(), DisplayPoint::new(DisplayRow(10), 0), 0.)
        );

        workspace
            .update(cx, |w, cx| w.go_back(w.active_pane().downgrade(), cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file1.clone(), DisplayPoint::new(DisplayRow(0), 0), 0.)
        );

        // Go back one more time and ensure we don't navigate past the first item in the history.
        workspace
            .update(cx, |w, cx| w.go_back(w.active_pane().downgrade(), cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file1.clone(), DisplayPoint::new(DisplayRow(0), 0), 0.)
        );

        workspace
            .update(cx, |w, cx| w.go_forward(w.active_pane().downgrade(), cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file1.clone(), DisplayPoint::new(DisplayRow(10), 0), 0.)
        );

        workspace
            .update(cx, |w, cx| w.go_forward(w.active_pane().downgrade(), cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file2.clone(), DisplayPoint::new(DisplayRow(0), 0), 0.)
        );

        // Go forward to an item that has been closed, ensuring it gets re-opened at the same
        // location.
        workspace
            .update(cx, |_, cx| {
                pane.update(cx, |pane, cx| {
                    let editor3_id = editor3.entity_id();
                    drop(editor3);
                    pane.close_item_by_id(editor3_id, SaveIntent::Close, cx)
                })
            })
            .unwrap()
            .await
            .unwrap();
        workspace
            .update(cx, |w, cx| w.go_forward(w.active_pane().downgrade(), cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file3.clone(), DisplayPoint::new(DisplayRow(0), 0), 0.)
        );

        workspace
            .update(cx, |w, cx| w.go_forward(w.active_pane().downgrade(), cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file3.clone(), DisplayPoint::new(DisplayRow(16), 0), 12.5)
        );

        workspace
            .update(cx, |w, cx| w.go_back(w.active_pane().downgrade(), cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file3.clone(), DisplayPoint::new(DisplayRow(0), 0), 0.)
        );

        // Go back to an item that has been closed and removed from disk
        workspace
            .update(cx, |_, cx| {
                pane.update(cx, |pane, cx| {
                    let editor2_id = editor2.entity_id();
                    drop(editor2);
                    pane.close_item_by_id(editor2_id, SaveIntent::Close, cx)
                })
            })
            .unwrap()
            .await
            .unwrap();
        app_state
            .fs
            .remove_file(Path::new("/root/a/file2"), Default::default())
            .await
            .unwrap();
        cx.background_executor.run_until_parked();

        workspace
            .update(cx, |w, cx| w.go_back(w.active_pane().downgrade(), cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file2.clone(), DisplayPoint::new(DisplayRow(0), 0), 0.)
        );
        workspace
            .update(cx, |w, cx| w.go_forward(w.active_pane().downgrade(), cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file3.clone(), DisplayPoint::new(DisplayRow(0), 0), 0.)
        );

        // Modify file to collapse multiple nav history entries into the same location.
        // Ensure we don't visit the same location twice when navigating.
        workspace
            .update(cx, |_, cx| {
                editor1.update(cx, |editor, cx| {
                    editor.change_selections(None, cx, |s| {
                        s.select_display_ranges([DisplayPoint::new(DisplayRow(15), 0)
                            ..DisplayPoint::new(DisplayRow(15), 0)])
                    })
                });
            })
            .unwrap();
        for _ in 0..5 {
            workspace
                .update(cx, |_, cx| {
                    editor1.update(cx, |editor, cx| {
                        editor.change_selections(None, cx, |s| {
                            s.select_display_ranges([DisplayPoint::new(DisplayRow(3), 0)
                                ..DisplayPoint::new(DisplayRow(3), 0)])
                        });
                    });
                })
                .unwrap();

            workspace
                .update(cx, |_, cx| {
                    editor1.update(cx, |editor, cx| {
                        editor.change_selections(None, cx, |s| {
                            s.select_display_ranges([DisplayPoint::new(DisplayRow(13), 0)
                                ..DisplayPoint::new(DisplayRow(13), 0)])
                        })
                    });
                })
                .unwrap();
        }
        workspace
            .update(cx, |_, cx| {
                editor1.update(cx, |editor, cx| {
                    editor.transact(cx, |editor, cx| {
                        editor.change_selections(None, cx, |s| {
                            s.select_display_ranges([DisplayPoint::new(DisplayRow(2), 0)
                                ..DisplayPoint::new(DisplayRow(14), 0)])
                        });
                        editor.insert("", cx);
                    })
                });
            })
            .unwrap();

        workspace
            .update(cx, |_, cx| {
                editor1.update(cx, |editor, cx| {
                    editor.change_selections(None, cx, |s| {
                        s.select_display_ranges([DisplayPoint::new(DisplayRow(1), 0)
                            ..DisplayPoint::new(DisplayRow(1), 0)])
                    })
                });
            })
            .unwrap();
        workspace
            .update(cx, |w, cx| w.go_back(w.active_pane().downgrade(), cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file1.clone(), DisplayPoint::new(DisplayRow(2), 0), 0.)
        );
        workspace
            .update(cx, |w, cx| w.go_back(w.active_pane().downgrade(), cx))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            active_location(&workspace, cx),
            (file1.clone(), DisplayPoint::new(DisplayRow(3), 0), 0.)
        );

        fn active_location(
            workspace: &WindowHandle<Workspace>,
            cx: &mut TestAppContext,
        ) -> (ProjectPath, DisplayPoint, f32) {
            workspace
                .update(cx, |workspace, cx| {
                    let item = workspace.active_item(cx).unwrap();
                    let editor = item.downcast::<Editor>().unwrap();
                    let (selections, scroll_position) = editor.update(cx, |editor, cx| {
                        (
                            editor.selections.display_ranges(cx),
                            editor.scroll_position(cx),
                        )
                    });
                    (
                        item.project_path(cx).unwrap(),
                        selections[0].start,
                        scroll_position.y,
                    )
                })
                .unwrap()
        }
    }

    #[gpui::test]
    async fn test_reopening_closed_items(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        app_state
            .fs
            .as_fake()
            .insert_tree(
                "/root",
                json!({
                    "a": {
                        "file1": "",
                        "file2": "",
                        "file3": "",
                        "file4": "",
                    },
                }),
            )
            .await;

        let project = Project::test(app_state.fs.clone(), ["/root".as_ref()], cx).await;
        project.update(cx, |project, _cx| {
            project.languages().add(markdown_language())
        });
        let workspace = cx.add_window(|cx| Workspace::test_new(project, cx));
        let pane = workspace
            .read_with(cx, |workspace, _| workspace.active_pane().clone())
            .unwrap();

        let entries = cx.update(|cx| workspace.root(cx).unwrap().file_project_paths(cx));
        let file1 = entries[0].clone();
        let file2 = entries[1].clone();
        let file3 = entries[2].clone();
        let file4 = entries[3].clone();

        let file1_item_id = workspace
            .update(cx, |w, cx| w.open_path(file1.clone(), None, true, cx))
            .unwrap()
            .await
            .unwrap()
            .item_id();
        let file2_item_id = workspace
            .update(cx, |w, cx| w.open_path(file2.clone(), None, true, cx))
            .unwrap()
            .await
            .unwrap()
            .item_id();
        let file3_item_id = workspace
            .update(cx, |w, cx| w.open_path(file3.clone(), None, true, cx))
            .unwrap()
            .await
            .unwrap()
            .item_id();
        let file4_item_id = workspace
            .update(cx, |w, cx| w.open_path(file4.clone(), None, true, cx))
            .unwrap()
            .await
            .unwrap()
            .item_id();
        assert_eq!(active_path(&workspace, cx), Some(file4.clone()));

        // Close all the pane items in some arbitrary order.
        workspace
            .update(cx, |_, cx| {
                pane.update(cx, |pane, cx| {
                    pane.close_item_by_id(file1_item_id, SaveIntent::Close, cx)
                })
            })
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file4.clone()));

        workspace
            .update(cx, |_, cx| {
                pane.update(cx, |pane, cx| {
                    pane.close_item_by_id(file4_item_id, SaveIntent::Close, cx)
                })
            })
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file3.clone()));

        workspace
            .update(cx, |_, cx| {
                pane.update(cx, |pane, cx| {
                    pane.close_item_by_id(file2_item_id, SaveIntent::Close, cx)
                })
            })
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file3.clone()));
        workspace
            .update(cx, |_, cx| {
                pane.update(cx, |pane, cx| {
                    pane.close_item_by_id(file3_item_id, SaveIntent::Close, cx)
                })
            })
            .unwrap()
            .await
            .unwrap();

        assert_eq!(active_path(&workspace, cx), None);

        // Reopen all the closed items, ensuring they are reopened in the same order
        // in which they were closed.
        workspace
            .update(cx, Workspace::reopen_closed_item)
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file3.clone()));

        workspace
            .update(cx, Workspace::reopen_closed_item)
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file2.clone()));

        workspace
            .update(cx, Workspace::reopen_closed_item)
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file4.clone()));

        workspace
            .update(cx, Workspace::reopen_closed_item)
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file1.clone()));

        // Reopening past the last closed item is a no-op.
        workspace
            .update(cx, Workspace::reopen_closed_item)
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file1.clone()));

        // Reopening closed items doesn't interfere with navigation history.
        workspace
            .update(cx, |workspace, cx| {
                workspace.go_back(workspace.active_pane().downgrade(), cx)
            })
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file4.clone()));

        workspace
            .update(cx, |workspace, cx| {
                workspace.go_back(workspace.active_pane().downgrade(), cx)
            })
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file2.clone()));

        workspace
            .update(cx, |workspace, cx| {
                workspace.go_back(workspace.active_pane().downgrade(), cx)
            })
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file3.clone()));

        workspace
            .update(cx, |workspace, cx| {
                workspace.go_back(workspace.active_pane().downgrade(), cx)
            })
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file4.clone()));

        workspace
            .update(cx, |workspace, cx| {
                workspace.go_back(workspace.active_pane().downgrade(), cx)
            })
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file3.clone()));

        workspace
            .update(cx, |workspace, cx| {
                workspace.go_back(workspace.active_pane().downgrade(), cx)
            })
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file2.clone()));

        workspace
            .update(cx, |workspace, cx| {
                workspace.go_back(workspace.active_pane().downgrade(), cx)
            })
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file1.clone()));

        workspace
            .update(cx, |workspace, cx| {
                workspace.go_back(workspace.active_pane().downgrade(), cx)
            })
            .unwrap()
            .await
            .unwrap();
        assert_eq!(active_path(&workspace, cx), Some(file1.clone()));

        fn active_path(
            workspace: &WindowHandle<Workspace>,
            cx: &TestAppContext,
        ) -> Option<ProjectPath> {
            workspace
                .read_with(cx, |workspace, cx| {
                    let item = workspace.active_item(cx)?;
                    item.project_path(cx)
                })
                .unwrap()
        }
    }

    fn init_keymap_test(cx: &mut TestAppContext) -> Arc<AppState> {
        cx.update(|cx| {
            let app_state = AppState::test(cx);

            theme::init(theme::LoadThemes::JustBase, cx);
            client::init(&app_state.client, cx);
            language::init(cx);
            workspace::init(app_state.clone(), cx);
            welcome::init(cx);
            Project::init_settings(cx);
            app_state
        })
    }

    #[gpui::test]
    async fn test_base_keymap(cx: &mut gpui::TestAppContext) {
        let executor = cx.executor();
        let app_state = init_keymap_test(cx);
        let project = Project::test(app_state.fs.clone(), [], cx).await;
        let workspace = cx.add_window(|cx| Workspace::test_new(project.clone(), cx));

        actions!(test1, [A, B]);
        // From the Atom keymap
        use workspace::ActivatePreviousPane;
        // From the JetBrains keymap
        use workspace::ActivatePrevItem;

        app_state
            .fs
            .save(
                "/settings.json".as_ref(),
                &r#"{"base_keymap": "Atom"}"#.into(),
                Default::default(),
            )
            .await
            .unwrap();

        app_state
            .fs
            .save(
                "/keymap.json".as_ref(),
                &r#"[{"bindings": {"backspace": "test1::A"}}]"#.into(),
                Default::default(),
            )
            .await
            .unwrap();
        executor.run_until_parked();
        cx.update(|cx| {
            let settings_rx = watch_config_file(
                &executor,
                app_state.fs.clone(),
                PathBuf::from("/settings.json"),
            );
            let keymap_rx = watch_config_file(
                &executor,
                app_state.fs.clone(),
                PathBuf::from("/keymap.json"),
            );
            handle_settings_file_changes(settings_rx, cx, |_, _| {});
            handle_keymap_file_changes(keymap_rx, cx, |_, _| {});
        });
        workspace
            .update(cx, |workspace, cx| {
                workspace.register_action(|_, _: &A, _cx| {});
                workspace.register_action(|_, _: &B, _cx| {});
                workspace.register_action(|_, _: &ActivatePreviousPane, _cx| {});
                workspace.register_action(|_, _: &ActivatePrevItem, _cx| {});
                cx.notify();
            })
            .unwrap();
        executor.run_until_parked();
        // Test loading the keymap base at all
        assert_key_bindings_for(
            workspace.into(),
            cx,
            vec![("backspace", &A), ("k", &ActivatePreviousPane)],
            line!(),
        );

        // Test modifying the users keymap, while retaining the base keymap
        app_state
            .fs
            .save(
                "/keymap.json".as_ref(),
                &r#"[{"bindings": {"backspace": "test1::B"}}]"#.into(),
                Default::default(),
            )
            .await
            .unwrap();

        executor.run_until_parked();

        assert_key_bindings_for(
            workspace.into(),
            cx,
            vec![("backspace", &B), ("k", &ActivatePreviousPane)],
            line!(),
        );

        // Test modifying the base, while retaining the users keymap
        app_state
            .fs
            .save(
                "/settings.json".as_ref(),
                &r#"{"base_keymap": "JetBrains"}"#.into(),
                Default::default(),
            )
            .await
            .unwrap();

        executor.run_until_parked();

        assert_key_bindings_for(
            workspace.into(),
            cx,
            vec![("backspace", &B), ("[", &ActivatePrevItem)],
            line!(),
        );
    }

    #[gpui::test]
    async fn test_disabled_keymap_binding(cx: &mut gpui::TestAppContext) {
        let executor = cx.executor();
        let app_state = init_keymap_test(cx);
        let project = Project::test(app_state.fs.clone(), [], cx).await;
        let workspace = cx.add_window(|cx| Workspace::test_new(project.clone(), cx));

        actions!(test2, [A, B]);
        // From the Atom keymap
        use workspace::ActivatePreviousPane;
        // From the JetBrains keymap
        use diagnostics::Deploy;

        workspace
            .update(cx, |workspace, _| {
                workspace.register_action(|_, _: &A, _cx| {});
                workspace.register_action(|_, _: &B, _cx| {});
                workspace.register_action(|_, _: &Deploy, _cx| {});
            })
            .unwrap();
        app_state
            .fs
            .save(
                "/settings.json".as_ref(),
                &r#"{"base_keymap": "Atom"}"#.into(),
                Default::default(),
            )
            .await
            .unwrap();
        app_state
            .fs
            .save(
                "/keymap.json".as_ref(),
                &r#"[{"bindings": {"backspace": "test2::A"}}]"#.into(),
                Default::default(),
            )
            .await
            .unwrap();

        cx.update(|cx| {
            let settings_rx = watch_config_file(
                &executor,
                app_state.fs.clone(),
                PathBuf::from("/settings.json"),
            );
            let keymap_rx = watch_config_file(
                &executor,
                app_state.fs.clone(),
                PathBuf::from("/keymap.json"),
            );

            handle_settings_file_changes(settings_rx, cx, |_, _| {});
            handle_keymap_file_changes(keymap_rx, cx, |_, _| {});
        });

        cx.background_executor.run_until_parked();

        cx.background_executor.run_until_parked();
        // Test loading the keymap base at all
        assert_key_bindings_for(
            workspace.into(),
            cx,
            vec![("backspace", &A), ("k", &ActivatePreviousPane)],
            line!(),
        );

        // Test disabling the key binding for the base keymap
        app_state
            .fs
            .save(
                "/keymap.json".as_ref(),
                &r#"[{"bindings": {"backspace": null}}]"#.into(),
                Default::default(),
            )
            .await
            .unwrap();

        cx.background_executor.run_until_parked();

        assert_key_bindings_for(
            workspace.into(),
            cx,
            vec![("k", &ActivatePreviousPane)],
            line!(),
        );

        // Test modifying the base, while retaining the users keymap
        app_state
            .fs
            .save(
                "/settings.json".as_ref(),
                &r#"{"base_keymap": "JetBrains"}"#.into(),
                Default::default(),
            )
            .await
            .unwrap();

        cx.background_executor.run_until_parked();

        assert_key_bindings_for(workspace.into(), cx, vec![("6", &Deploy)], line!());
    }

    #[gpui::test]
    fn test_bundled_settings_and_themes(cx: &mut AppContext) {
        cx.text_system()
            .add_fonts(vec![
                Assets
                    .load("fonts/plex-mono/ZedPlexMono-Regular.ttf")
                    .unwrap()
                    .unwrap(),
                Assets
                    .load("fonts/plex-sans/ZedPlexSans-Regular.ttf")
                    .unwrap()
                    .unwrap(),
            ])
            .unwrap();
        let themes = ThemeRegistry::default();
        settings::init(cx);
        theme::init(theme::LoadThemes::JustBase, cx);

        let mut has_default_theme = false;
        for theme_name in themes.list().into_iter().map(|meta| meta.name) {
            let theme = themes.get(&theme_name).unwrap();
            assert_eq!(theme.name, theme_name);
            if theme.name == ThemeSettings::get(None, cx).active_theme.name {
                has_default_theme = true;
            }
        }
        assert!(has_default_theme);
    }

    #[gpui::test]
    async fn test_bundled_languages(cx: &mut TestAppContext) {
        env_logger::builder().is_test(true).try_init().ok();
        let settings = cx.update(SettingsStore::test);
        cx.set_global(settings);
        let languages = LanguageRegistry::test(cx.executor());
        let languages = Arc::new(languages);
        let node_runtime = node_runtime::NodeRuntime::unavailable();
        cx.update(|cx| {
            languages::init(languages.clone(), node_runtime, cx);
        });
        for name in languages.language_names() {
            languages
                .language_for_name(&name)
                .await
                .with_context(|| format!("language name {name}"))
                .unwrap();
        }
        cx.run_until_parked();
    }

    pub(crate) fn init_test(cx: &mut TestAppContext) -> Arc<AppState> {
        init_test_with_state(cx, cx.update(AppState::test))
    }

    fn init_test_with_state(
        cx: &mut TestAppContext,
        mut app_state: Arc<AppState>,
    ) -> Arc<AppState> {
        cx.update(move |cx| {
            env_logger::builder().is_test(true).try_init().ok();

            let state = Arc::get_mut(&mut app_state).unwrap();
            state.build_window_options = build_window_options;

            app_state.languages.add(markdown_language());

            //vim_mode_setting::init(cx);
            theme::init(theme::LoadThemes::JustBase, cx);
            audio::init((), cx);
            channel::init(&app_state.client, app_state.user_store.clone(), cx);
            call::init(app_state.client.clone(), app_state.user_store.clone(), cx);
            notifications::init(app_state.client.clone(), app_state.user_store.clone(), cx);
            workspace::init(app_state.clone(), cx);
            Project::init_settings(cx);
            release_channel::init(SemanticVersion::default(), cx);
            command_palette::init(cx);
            language::init(cx);
            editor::init(cx);
            collab_ui::init(&app_state, cx);
            git_ui::init(cx);
            project_panel::init((), cx);
            outline_panel::init((), cx);
            terminal_view::init(cx);
            copilot::copilot_chat::init(
                app_state.fs.clone(),
                app_state.client.http_client().clone(),
                cx,
            );
            language_model::init(cx);
            language_models::init(
                app_state.user_store.clone(),
                app_state.client.clone(),
                app_state.fs.clone(),
                cx,
            );
            let prompt_builder =
                assistant::init(app_state.fs.clone(), app_state.client.clone(), false, cx);
            repl::init(app_state.fs.clone(), cx);
            repl::notebook::init(cx);
            tasks_ui::init(cx);
            initialize_workspace(app_state.clone(), prompt_builder, cx);
            search::init(cx);
            app_state
        })
    }

    fn rust_lang() -> Arc<language::Language> {
        Arc::new(language::Language::new(
            language::LanguageConfig {
                name: "Rust".into(),
                matcher: LanguageMatcher {
                    path_suffixes: vec!["rs".to_string()],
                    ..Default::default()
                },
                ..Default::default()
            },
            Some(tree_sitter_rust::LANGUAGE.into()),
        ))
    }

    fn markdown_language() -> Arc<language::Language> {
        Arc::new(language::Language::new(
            language::LanguageConfig {
                name: "Markdown".into(),
                matcher: LanguageMatcher {
                    path_suffixes: vec!["md".to_string()],
                    ..Default::default()
                },
                ..Default::default()
            },
            Some(tree_sitter_md::LANGUAGE.into()),
        ))
    }

    #[track_caller]
    fn assert_key_bindings_for(
        window: AnyWindowHandle,
        cx: &TestAppContext,
        actions: Vec<(&'static str, &dyn Action)>,
        line: u32,
    ) {
        let available_actions = cx
            .update(|cx| window.update(cx, |_, cx| cx.available_actions()))
            .unwrap();
        for (key, action) in actions {
            let bindings = cx
                .update(|cx| window.update(cx, |_, cx| cx.bindings_for_action(action)))
                .unwrap();
            // assert that...
            assert!(
                available_actions.iter().any(|bound_action| {
                    // actions match...
                    bound_action.partial_eq(action)
                }),
                "On {} Failed to find {}",
                line,
                action.name(),
            );
            assert!(
                // and key strokes contain the given key
                bindings
                    .into_iter()
                    .any(|binding| binding.keystrokes().iter().any(|k| k.key == key)),
                "On {} Failed to find {} with key binding {}",
                line,
                action.name(),
                key
            );
        }
    }
}
