mod components;
mod extension_suggest;
mod extension_version_selector;

use std::ops::DerefMut;
use std::sync::OnceLock;
use std::time::Duration;
use std::{ops::Range, sync::Arc};

use client::ExtensionMetadata;
use collections::{BTreeMap, BTreeSet};
use editor::{Editor, EditorElement, EditorStyle};
use extension_host::{ExtensionManifest, ExtensionOperation, ExtensionStore};
use fuzzy::{match_strings, StringMatchCandidate};
use gpui::{
    actions, uniform_list, Action, AppContext, ClipboardItem, EventEmitter, Flatten, FocusableView,
    InteractiveElement, KeyContext, ParentElement, Render, Styled, Task, TextStyle,
    UniformListScrollHandle, View, ViewContext, VisualContext, WeakView, WindowContext,
};
use num_format::{Locale, ToFormattedString};
use project::DirectoryLister;
use release_channel::ReleaseChannel;
use settings::Settings;
use theme::ThemeSettings;
use ui::{prelude::*,ContextMenu, PopoverMenu, ToggleButton, Tooltip};
// use vim_mode_setting::VimModeSetting;
use workspace::{
    item::{Item, ItemEvent},
    Workspace, WorkspaceId,
};

use crate::components::{ExtensionCard, FeatureUpsell};
use crate::extension_version_selector::{
    ExtensionVersionSelector, ExtensionVersionSelectorDelegate,
};

actions!(zed, [InstallDevExtension]);

pub fn init(cx: &mut AppContext) {
    cx.observe_new_views(move |workspace: &mut Workspace, cx| {
        workspace
            .register_action(move |workspace, _: &zed_actions::Extensions, cx| {
                let existing = workspace
                    .active_pane()
                    .read(cx)
                    .items()
                    .find_map(|item| item.downcast::<ExtensionsPage>());

                if let Some(existing) = existing {
                    workspace.activate_item(&existing, true, true, cx);
                } else {
                    let extensions_page = ExtensionsPage::new(workspace, cx);
                    workspace.add_item_to_active_pane(Box::new(extensions_page), None, true, cx)
                }
            })
            .register_action(move |workspace, _: &InstallDevExtension, cx| {
                let store = ExtensionStore::global(cx);
                let prompt = workspace.prompt_for_open_path(
                    gpui::PathPromptOptions {
                        files: false,
                        directories: true,
                        multiple: false,
                    },
                    DirectoryLister::Local(workspace.app_state().fs.clone()),
                    cx,
                );

                let workspace_handle = cx.view().downgrade();
                cx.deref_mut()
                    .spawn(|mut cx| async move {
                        let extension_path =
                            match Flatten::flatten(prompt.await.map_err(|e| e.into())) {
                                Ok(Some(mut paths)) => paths.pop()?,
                                Ok(None) => return None,
                                Err(err) => {
                                    workspace_handle
                                        .update(&mut cx, |workspace, cx| {
                                            workspace.show_portal_error(err.to_string(), cx);
                                        })
                                        .ok();
                                    return None;
                                }
                            };

                        store
                            .update(&mut cx, |store, cx| {
                                store
                                    .install_dev_extension(extension_path, cx)
                                    .detach_and_log_err(cx)
                            })
                            .ok()?;
                        Some(())
                    })
                    .detach();
            });

        cx.subscribe(workspace.project(), |_, _, event, cx| {
            if let project::Event::LanguageNotFound(buffer) = event {
                extension_suggest::suggest(buffer.clone(), cx);
            }
        })
        .detach();
    })
    .detach();
}

#[derive(Clone)]
pub enum ExtensionStatus {
    NotInstalled,
    Installing,
    Upgrading,
    Installed(Arc<str>),
    Removing,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
enum ExtensionFilter {
    All,
    Installed,
    NotInstalled,
}

impl ExtensionFilter {
    pub fn include_dev_extensions(&self) -> bool {
        match self {
            Self::All | Self::Installed => true,
            Self::NotInstalled => false,
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
enum Feature {
    Git,
    OpenIn,
    //Vim,
    LanguageBash,
    LanguageC,
    LanguageCpp,
    LanguageGo,
    LanguagePython,
    LanguageReact,
    LanguageRust,
    LanguageTypescript,
}

fn keywords_by_feature() -> &'static BTreeMap<Feature, Vec<&'static str>> {
    static KEYWORDS_BY_FEATURE: OnceLock<BTreeMap<Feature, Vec<&'static str>>> = OnceLock::new();
    KEYWORDS_BY_FEATURE.get_or_init(|| {
        BTreeMap::from_iter([
            (Feature::Git, vec!["git"]),
            (
                Feature::OpenIn,
                vec![
                    "github",
                    "gitlab",
                    "bitbucket",
                    "codeberg",
                    "sourcehut",
                    "permalink",
                    "link",
                    "open in",
                ],
            ),
            //(Feature::Vim, vec!["vim"]),
            (Feature::LanguageBash, vec!["sh", "bash"]),
            (Feature::LanguageC, vec!["c", "clang"]),
            (Feature::LanguageCpp, vec!["c++", "cpp", "clang"]),
            (Feature::LanguageGo, vec!["go", "golang"]),
            (Feature::LanguagePython, vec!["python", "py"]),
            (Feature::LanguageReact, vec!["react"]),
            (Feature::LanguageRust, vec!["rust", "rs"]),
            (
                Feature::LanguageTypescript,
                vec!["type", "typescript", "ts"],
            ),
        ])
    })
}

pub struct ExtensionsPage {
    workspace: WeakView<Workspace>,
    list: UniformListScrollHandle,
    is_fetching_extensions: bool,
    filter: ExtensionFilter,
    remote_extension_entries: Vec<ExtensionMetadata>,
    dev_extension_entries: Vec<Arc<ExtensionManifest>>,
    filtered_remote_extension_indices: Vec<usize>,
    query_editor: View<Editor>,
    query_contains_error: bool,
    _subscriptions: [gpui::Subscription; 2],
    extension_fetch_task: Option<Task<()>>,
    upsells: BTreeSet<Feature>,
}

impl ExtensionsPage {
    pub fn new(workspace: &Workspace, cx: &mut ViewContext<Workspace>) -> View<Self> {
        cx.new_view(|cx: &mut ViewContext<Self>| {
            let store = ExtensionStore::global(cx);
            let workspace_handle = workspace.weak_handle();
            let subscriptions = [
                cx.observe(&store, |_, _, cx| cx.notify()),
                cx.subscribe(&store, move |this, _, event, cx| match event {
                    extension_host::Event::ExtensionsUpdated => this.fetch_extensions_debounced(cx),
                    extension_host::Event::ExtensionInstalled(extension_id) => {
                        this.on_extension_installed(workspace_handle.clone(), extension_id, cx)
                    }
                    _ => {}
                }),
            ];

            let query_editor = cx.new_view(|cx| {
                let mut input = Editor::single_line(cx);
                input.set_placeholder_text("Search extensions...", cx);
                input
            });
            cx.subscribe(&query_editor, Self::on_query_change).detach();

            let mut this = Self {
                workspace: workspace.weak_handle(),
                list: UniformListScrollHandle::new(),
                is_fetching_extensions: false,
                filter: ExtensionFilter::All,
                dev_extension_entries: Vec::new(),
                filtered_remote_extension_indices: Vec::new(),
                remote_extension_entries: Vec::new(),
                query_contains_error: false,
                extension_fetch_task: None,
                _subscriptions: subscriptions,
                query_editor,
                upsells: BTreeSet::default(),
            };
            this.fetch_extensions(None, cx);
            this
        })
    }

    fn on_extension_installed(
        &mut self,
        workspace: WeakView<Workspace>,
        extension_id: &str,
        cx: &mut ViewContext<Self>,
    ) {
        let extension_store = ExtensionStore::global(cx).read(cx);
        let themes = extension_store
            .extension_themes(extension_id)
            .map(|name| name.to_string())
            .collect::<Vec<_>>();
        if !themes.is_empty() {
            workspace
                .update(cx, |_workspace, cx| {
                    cx.dispatch_action(
                        zed_actions::theme_selector::Toggle {
                            themes_filter: Some(themes),
                        }
                        .boxed_clone(),
                    );
                })
                .ok();
        }
    }

    /// Returns whether a dev extension currently exists for the extension with the given ID.
    fn dev_extension_exists(extension_id: &str, cx: &mut ViewContext<Self>) -> bool {
        let extension_store = ExtensionStore::global(cx).read(cx);

        extension_store
            .dev_extensions()
            .any(|dev_extension| dev_extension.id.as_ref() == extension_id)
    }

    fn extension_status(extension_id: &str, cx: &mut ViewContext<Self>) -> ExtensionStatus {
        let extension_store = ExtensionStore::global(cx).read(cx);

        match extension_store.outstanding_operations().get(extension_id) {
            Some(ExtensionOperation::Install) => ExtensionStatus::Installing,
            Some(ExtensionOperation::Remove) => ExtensionStatus::Removing,
            Some(ExtensionOperation::Upgrade) => ExtensionStatus::Upgrading,
            None => match extension_store.installed_extensions().get(extension_id) {
                Some(extension) => ExtensionStatus::Installed(extension.manifest.version.clone()),
                None => ExtensionStatus::NotInstalled,
            },
        }
    }

    fn filter_extension_entries(&mut self, cx: &mut ViewContext<Self>) {
        self.filtered_remote_extension_indices.clear();
        self.filtered_remote_extension_indices.extend(
            self.remote_extension_entries
                .iter()
                .enumerate()
                .filter(|(_, extension)| match self.filter {
                    ExtensionFilter::All => true,
                    ExtensionFilter::Installed => {
                        let status = Self::extension_status(&extension.id, cx);
                        matches!(status, ExtensionStatus::Installed(_))
                    }
                    ExtensionFilter::NotInstalled => {
                        let status = Self::extension_status(&extension.id, cx);

                        matches!(status, ExtensionStatus::NotInstalled)
                    }
                })
                .map(|(ix, _)| ix),
        );
        cx.notify();
    }

    fn fetch_extensions(&mut self, search: Option<String>, cx: &mut ViewContext<Self>) {
        self.is_fetching_extensions = true;
        cx.notify();

        let extension_store = ExtensionStore::global(cx);

        let dev_extensions = extension_store.update(cx, |store, _| {
            store.dev_extensions().cloned().collect::<Vec<_>>()
        });

        let remote_extensions = extension_store.update(cx, |store, cx| {
            store.fetch_extensions(search.as_deref(), cx)
        });

        cx.spawn(move |this, mut cx| async move {
            let dev_extensions = if let Some(search) = search {
                let match_candidates = dev_extensions
                    .iter()
                    .enumerate()
                    .map(|(ix, manifest)| StringMatchCandidate::new(ix, &manifest.name))
                    .collect::<Vec<_>>();

                let matches = match_strings(
                    &match_candidates,
                    &search,
                    false,
                    match_candidates.len(),
                    &Default::default(),
                    cx.background_executor().clone(),
                )
                .await;
                matches
                    .into_iter()
                    .map(|mat| dev_extensions[mat.candidate_id].clone())
                    .collect()
            } else {
                dev_extensions
            };

            let fetch_result = remote_extensions.await;
            this.update(&mut cx, |this, cx| {
                cx.notify();
                this.dev_extension_entries = dev_extensions;
                this.is_fetching_extensions = false;
                this.remote_extension_entries = fetch_result?;
                this.filter_extension_entries(cx);
                anyhow::Ok(())
            })?
        })
        .detach_and_log_err(cx);
    }

    fn render_extensions(
        &mut self,
        range: Range<usize>,
        cx: &mut ViewContext<Self>,
    ) -> Vec<ExtensionCard> {
        let dev_extension_entries_len = if self.filter.include_dev_extensions() {
            self.dev_extension_entries.len()
        } else {
            0
        };
        range
            .map(|ix| {
                if ix < dev_extension_entries_len {
                    let extension = &self.dev_extension_entries[ix];
                    self.render_dev_extension(extension, cx)
                } else {
                    let extension_ix =
                        self.filtered_remote_extension_indices[ix - dev_extension_entries_len];
                    let extension = &self.remote_extension_entries[extension_ix];
                    self.render_remote_extension(extension, cx)
                }
            })
            .collect()
    }

    fn render_dev_extension(
        &self,
        extension: &ExtensionManifest,
        cx: &mut ViewContext<Self>,
    ) -> ExtensionCard {
        let status = Self::extension_status(&extension.id, cx);

        let repository_url = extension.repository.clone();

        ExtensionCard::new()
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_end()
                            .child(Headline::new(extension.name.clone()).size(HeadlineSize::Medium))
                            .child(
                                Headline::new(format!("v{}", extension.version))
                                    .size(HeadlineSize::XSmall),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_between()
                            .child(
                                Button::new(
                                    SharedString::from(format!("rebuild-{}", extension.id)),
                                    "Rebuild",
                                )
                                .on_click({
                                    let extension_id = extension.id.clone();
                                    move |_, cx| {
                                        ExtensionStore::global(cx).update(cx, |store, cx| {
                                            store.rebuild_dev_extension(extension_id.clone(), cx)
                                        });
                                    }
                                })
                                .color(Color::Accent)
                                .disabled(matches!(status, ExtensionStatus::Upgrading)),
                            )
                            .child(
                                Button::new(SharedString::from(extension.id.clone()), "Uninstall")
                                    .on_click({
                                        let extension_id = extension.id.clone();
                                        move |_, cx| {
                                            ExtensionStore::global(cx).update(cx, |store, cx| {
                                                store.uninstall_extension(extension_id.clone(), cx)
                                            });
                                        }
                                    })
                                    .color(Color::Accent)
                                    .disabled(matches!(status, ExtensionStatus::Removing)),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .justify_between()
                    .child(
                        Label::new(format!(
                            "{}: {}",
                            if extension.authors.len() > 1 {
                                "Authors"
                            } else {
                                "Author"
                            },
                            extension.authors.join(", ")
                        ))
                        .size(LabelSize::Small)
                        .text_ellipsis(),
                    )
                    .child(Label::new("<>").size(LabelSize::Small)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .justify_between()
                    .children(extension.description.as_ref().map(|description| {
                        Label::new(description.clone())
                            .size(LabelSize::Small)
                            .color(Color::Default)
                            .text_ellipsis()
                    }))
                    .children(repository_url.map(|repository_url| {
                        IconButton::new(
                            SharedString::from(format!("repository-{}", extension.id)),
                            IconName::Github,
                        )
                        .icon_color(Color::Accent)
                        .icon_size(IconSize::Small)
                        .style(ButtonStyle::Filled)
                        .on_click(cx.listener({
                            let repository_url = repository_url.clone();
                            move |_, _, cx| {
                                cx.open_url(&repository_url);
                            }
                        }))
                        .tooltip(move |cx| Tooltip::text(repository_url.clone(), cx))
                    })),
            )
    }

    fn render_remote_extension(
        &self,
        extension: &ExtensionMetadata,
        cx: &mut ViewContext<Self>,
    ) -> ExtensionCard {
        let this = cx.view().clone();
        let status = Self::extension_status(&extension.id, cx);
        let has_dev_extension = Self::dev_extension_exists(&extension.id, cx);

        let extension_id = extension.id.clone();
        let (install_or_uninstall_button, upgrade_button) =
            self.buttons_for_entry(extension, &status, has_dev_extension, cx);
        let version = extension.manifest.version.clone();
        let repository_url = extension.manifest.repository.clone();

        let installed_version = match status {
            ExtensionStatus::Installed(installed_version) => Some(installed_version),
            _ => None,
        };

        ExtensionCard::new()
            .overridden_by_dev_extension(has_dev_extension)
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_end()
                            .child(
                                Headline::new(extension.manifest.name.clone())
                                    .size(HeadlineSize::Medium),
                            )
                            .child(Headline::new(format!("v{version}")).size(HeadlineSize::XSmall))
                            .children(
                                installed_version
                                    .filter(|installed_version| *installed_version != version)
                                    .map(|installed_version| {
                                        Headline::new(format!("(v{installed_version} installed)",))
                                            .size(HeadlineSize::XSmall)
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_between()
                            .children(upgrade_button)
                            .child(install_or_uninstall_button),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .justify_between()
                    .child(
                        Label::new(format!(
                            "{}: {}",
                            if extension.manifest.authors.len() > 1 {
                                "Authors"
                            } else {
                                "Author"
                            },
                            extension.manifest.authors.join(", ")
                        ))
                        .size(LabelSize::Small)
                        .text_ellipsis(),
                    )
                    .child(
                        Label::new(format!(
                            "Downloads: {}",
                            extension.download_count.to_formatted_string(&Locale::en)
                        ))
                        .size(LabelSize::Small),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .justify_between()
                    .children(extension.manifest.description.as_ref().map(|description| {
                        Label::new(description.clone())
                            .size(LabelSize::Small)
                            .color(Color::Default)
                            .text_ellipsis()
                    }))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                IconButton::new(
                                    SharedString::from(format!("repository-{}", extension.id)),
                                    IconName::Github,
                                )
                                .icon_color(Color::Accent)
                                .icon_size(IconSize::Small)
                                .style(ButtonStyle::Filled)
                                .on_click(cx.listener({
                                    let repository_url = repository_url.clone();
                                    move |_, _, cx| {
                                        cx.open_url(&repository_url);
                                    }
                                }))
                                .tooltip(move |cx| Tooltip::text(repository_url.clone(), cx)),
                            )
                            .child(
                                PopoverMenu::new(SharedString::from(format!(
                                    "more-{}",
                                    extension.id
                                )))
                                .trigger(
                                    IconButton::new(
                                        SharedString::from(format!("more-{}", extension.id)),
                                        IconName::Ellipsis,
                                    )
                                    .icon_color(Color::Accent)
                                    .icon_size(IconSize::Small)
                                    .style(ButtonStyle::Filled),
                                )
                                .menu(move |cx| {
                                    Some(Self::render_remote_extension_context_menu(
                                        &this,
                                        extension_id.clone(),
                                        cx,
                                    ))
                                }),
                            ),
                    ),
            )
    }

    fn render_remote_extension_context_menu(
        this: &View<Self>,
        extension_id: Arc<str>,
        cx: &mut WindowContext,
    ) -> View<ContextMenu> {
        let context_menu = ContextMenu::build(cx, |context_menu, cx| {
            context_menu
                .entry(
                    "Install Another Version...",
                    None,
                    cx.handler_for(this, {
                        let extension_id = extension_id.clone();
                        move |this, cx| this.show_extension_version_list(extension_id.clone(), cx)
                    }),
                )
                .entry("Copy Extension ID", None, {
                    let extension_id = extension_id.clone();
                    move |cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(extension_id.to_string()));
                    }
                })
        });

        context_menu
    }

    fn show_extension_version_list(&mut self, extension_id: Arc<str>, cx: &mut ViewContext<Self>) {
        let Some(workspace) = self.workspace.upgrade() else {
            return;
        };

        cx.spawn(move |this, mut cx| async move {
            let extension_versions_task = this.update(&mut cx, |_, cx| {
                let extension_store = ExtensionStore::global(cx);

                extension_store.update(cx, |store, cx| {
                    store.fetch_extension_versions(&extension_id, cx)
                })
            })?;

            let extension_versions = extension_versions_task.await?;

            workspace.update(&mut cx, |workspace, cx| {
                let fs = workspace.project().read(cx).fs().clone();
                workspace.toggle_modal(cx, |cx| {
                    let delegate = ExtensionVersionSelectorDelegate::new(
                        fs,
                        cx.view().downgrade(),
                        extension_versions,
                    );

                    ExtensionVersionSelector::new(delegate, cx)
                });
            })?;

            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }

    fn buttons_for_entry(
        &self,
        extension: &ExtensionMetadata,
        status: &ExtensionStatus,
        has_dev_extension: bool,
        cx: &mut ViewContext<Self>,
    ) -> (Button, Option<Button>) {
        let is_compatible =
            extension_host::is_version_compatible(ReleaseChannel::global(cx), extension);

        if has_dev_extension {
            // If we have a dev extension for the given extension, just treat it as uninstalled.
            // The button here is a placeholder, as it won't be interactable anyways.
            return (
                Button::new(SharedString::from(extension.id.clone()), "Install"),
                None,
            );
        }

        match status.clone() {
            ExtensionStatus::NotInstalled => (
                Button::new(SharedString::from(extension.id.clone()), "Install").on_click({
                    let extension_id = extension.id.clone();
                    move |_, cx| {
                        telemetry::event!("Extension Installed");
                        ExtensionStore::global(cx).update(cx, |store, cx| {
                            store.install_latest_extension(extension_id.clone(), cx)
                        });
                    }
                }),
                None,
            ),
            ExtensionStatus::Installing => (
                Button::new(SharedString::from(extension.id.clone()), "Install").disabled(true),
                None,
            ),
            ExtensionStatus::Upgrading => (
                Button::new(SharedString::from(extension.id.clone()), "Uninstall").disabled(true),
                Some(
                    Button::new(SharedString::from(extension.id.clone()), "Upgrade").disabled(true),
                ),
            ),
            ExtensionStatus::Installed(installed_version) => (
                Button::new(SharedString::from(extension.id.clone()), "Uninstall").on_click({
                    let extension_id = extension.id.clone();
                    move |_, cx| {
                        telemetry::event!("Extension Uninstalled", extension_id);
                        ExtensionStore::global(cx).update(cx, |store, cx| {
                            store.uninstall_extension(extension_id.clone(), cx)
                        });
                    }
                }),
                if installed_version == extension.manifest.version {
                    None
                } else {
                    Some(
                        Button::new(SharedString::from(extension.id.clone()), "Upgrade")
                            .when(!is_compatible, |upgrade_button| {
                                upgrade_button.disabled(true).tooltip({
                                    let version = extension.manifest.version.clone();
                                    move |cx| {
                                        Tooltip::text(
                                            format!(
                                                "v{version} is not compatible with this version of Zed.",
                                            ),
                                            cx,
                                        )
                                    }
                                })
                            })
                            .disabled(!is_compatible)
                            .on_click({
                                let extension_id = extension.id.clone();
                                let version = extension.manifest.version.clone();
                                move |_, cx| {
                                    telemetry::event!("Extension Installed", extension_id, version);
                                    ExtensionStore::global(cx).update(cx, |store, cx| {
                                        store
                                            .upgrade_extension(
                                                extension_id.clone(),
                                                version.clone(),
                                                cx,
                                            )
                                            .detach_and_log_err(cx)
                                    });
                                }
                            }),
                    )
                },
            ),
            ExtensionStatus::Removing => (
                Button::new(SharedString::from(extension.id.clone()), "Uninstall").disabled(true),
                None,
            ),
        }
    }

    fn render_search(&self, cx: &mut ViewContext<Self>) -> Div {
        let mut key_context = KeyContext::new_with_defaults();
        key_context.add("BufferSearchBar");

        let editor_border = if self.query_contains_error {
            Color::Error.color(cx)
        } else {
            cx.theme().colors().border
        };

        h_flex().w_full().gap_2().key_context(key_context).child(
            h_flex()
                .flex_1()
                .px_2()
                .py_1()
                .gap_2()
                .border_1()
                .border_color(editor_border)
                .min_w(rems_from_px(384.))
                .rounded_lg()
                .child(Icon::new(IconName::MagnifyingGlass))
                .child(self.render_text_input(&self.query_editor, cx)),
        )
    }

    fn render_text_input(&self, editor: &View<Editor>, cx: &ViewContext<Self>) -> impl IntoElement {
        let settings = ThemeSettings::get_global(cx);
        let text_style = TextStyle {
            color: if editor.read(cx).read_only(cx) {
                cx.theme().colors().text_disabled
            } else {
                cx.theme().colors().text
            },
            font_family: settings.ui_font.family.clone(),
            font_features: settings.ui_font.features.clone(),
            font_fallbacks: settings.ui_font.fallbacks.clone(),
            font_size: rems(0.875).into(),
            font_weight: settings.ui_font.weight,
            line_height: relative(1.3),
            ..Default::default()
        };

        EditorElement::new(
            editor,
            EditorStyle {
                background: cx.theme().colors().editor_background,
                local_player: cx.theme().players().local(),
                text: text_style,
                ..Default::default()
            },
        )
    }

    fn on_query_change(
        &mut self,
        _: View<Editor>,
        event: &editor::EditorEvent,
        cx: &mut ViewContext<Self>,
    ) {
        if let editor::EditorEvent::Edited { .. } = event {
            self.query_contains_error = false;
            self.fetch_extensions_debounced(cx);
            self.refresh_feature_upsells(cx);
        }
    }

    fn fetch_extensions_debounced(&mut self, cx: &mut ViewContext<ExtensionsPage>) {
        self.extension_fetch_task = Some(cx.spawn(|this, mut cx| async move {
            let search = this
                .update(&mut cx, |this, cx| this.search_query(cx))
                .ok()
                .flatten();

            // Only debounce the fetching of extensions if we have a search
            // query.
            //
            // If the search was just cleared then we can just reload the list
            // of extensions without a debounce, which allows us to avoid seeing
            // an intermittent flash of a "no extensions" state.
            if search.is_some() {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
            };

            this.update(&mut cx, |this, cx| {
                this.fetch_extensions(search, cx);
            })
            .ok();
        }));
    }

    pub fn search_query(&self, cx: &WindowContext) -> Option<String> {
        let search = self.query_editor.read(cx).text(cx);
        if search.trim().is_empty() {
            None
        } else {
            Some(search)
        }
    }

    fn render_empty_state(&self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let has_search = self.search_query(cx).is_some();

        let message = if self.is_fetching_extensions {
            "Loading extensions..."
        } else {
            match self.filter {
                ExtensionFilter::All => {
                    if has_search {
                        "No extensions that match your search."
                    } else {
                        "No extensions."
                    }
                }
                ExtensionFilter::Installed => {
                    if has_search {
                        "No installed extensions that match your search."
                    } else {
                        "No installed extensions."
                    }
                }
                ExtensionFilter::NotInstalled => {
                    if has_search {
                        "No not installed extensions that match your search."
                    } else {
                        "No not installed extensions."
                    }
                }
            }
        };

        Label::new(message)
    }

    /* fn update_settings<T: Settings>(
        &mut self,
        selection: &ToggleState,
        cx: &mut ViewContext<Self>,
        callback: impl 'static + Send + Fn(&mut T::FileContent, bool),
    ) {
        if let Some(workspace) = self.workspace.upgrade() {
            let fs = workspace.read(cx).app_state().fs.clone();
            let selection = *selection;
            settings::update_settings_file::<T>(fs, cx, move |settings, _| {
                let value = match selection {
                    ToggleState::Unselected => false,
                    ToggleState::Selected => true,
                    _ => return,
                };

                callback(settings, value)
            });
        }
    } */

    fn refresh_feature_upsells(&mut self, cx: &mut ViewContext<Self>) {
        let Some(search) = self.search_query(cx) else {
            self.upsells.clear();
            return;
        };

        let search = search.to_lowercase();
        let search_terms = search
            .split_whitespace()
            .map(|term| term.trim())
            .collect::<Vec<_>>();

        for (feature, keywords) in keywords_by_feature() {
            if keywords
                .iter()
                .any(|keyword| search_terms.contains(keyword))
            {
                self.upsells.insert(*feature);
            } else {
                self.upsells.remove(feature);
            }
        }
    }

    fn render_feature_upsells(&self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let upsells_count = self.upsells.len();

        v_flex().children(self.upsells.iter().enumerate().map(|(ix, feature)| {
            let upsell = match feature {
                Feature::Git => FeatureUpsell::new(
                    "Zed comes with basic Git support. More Git features are coming in the future.",
                )
                .docs_url("https://zed.dev/docs/git"),
                Feature::OpenIn => FeatureUpsell::new(
                    "Zed supports linking to a source line on GitHub and others.",
                ),
                /* .docs_url("https://zed.dev/docs/git#git-integrations"),
                Feature::Vim => FeatureUpsell::new("Vim support is built-in to Zed!")
                    .docs_url("https://zed.dev/docs/vim")
                    .child(CheckboxWithLabel::new(
                        "enable-vim",
                        Label::new("Enable vim mode"),
                        if VimModeSetting::get_global(cx).0 {
                            ui::ToggleState::Selected
                    } else {
                            ui::ToggleState::Unselected
                        },
                        cx.listener(move |this, selection, cx| {
                            telemetry::event!("Vim Mode Toggled", source = "Feature Upsell");
                            this.update_settings::<VimModeSetting>(
                                selection,
                                cx,
                                |setting, value| *setting = Some(value),
                            );
                        }),
                    )),*/
                Feature::LanguageBash => FeatureUpsell::new("Shell support is built-in to Zed!")
                    .docs_url("https://zed.dev/docs/languages/bash"),
                Feature::LanguageC => FeatureUpsell::new("C support is built-in to Zed!")
                    .docs_url("https://zed.dev/docs/languages/c"),
                Feature::LanguageCpp => FeatureUpsell::new("C++ support is built-in to Zed!")
                    .docs_url("https://zed.dev/docs/languages/cpp"),
                Feature::LanguageGo => FeatureUpsell::new("Go support is built-in to Zed!")
                    .docs_url("https://zed.dev/docs/languages/go"),
                Feature::LanguagePython => FeatureUpsell::new("Python support is built-in to Zed!")
                    .docs_url("https://zed.dev/docs/languages/python"),
                Feature::LanguageReact => FeatureUpsell::new("React support is built-in to Zed!")
                    .docs_url("https://zed.dev/docs/languages/typescript"),
                Feature::LanguageRust => FeatureUpsell::new("Rust support is built-in to Zed!")
                    .docs_url("https://zed.dev/docs/languages/rust"),
                Feature::LanguageTypescript => {
                    FeatureUpsell::new("Typescript support is built-in to Zed!")
                        .docs_url("https://zed.dev/docs/languages/typescript")
                }
            };

            upsell.when(ix < upsells_count, |upsell| upsell.border_b_1())
        }))
    }
}

impl Render for ExtensionsPage {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().colors().editor_background)
            .child(
                v_flex()
                    .gap_4()
                    .p_4()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .bg(cx.theme().colors().editor_background)
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .justify_between()
                            .child(Headline::new("Extensions").size(HeadlineSize::XLarge))
                            .child(
                                Button::new("install-dev-extension", "Install Dev Extension")
                                    .style(ButtonStyle::Filled)
                                    .size(ButtonSize::Large)
                                    .on_click(|_event, cx| {
                                        cx.dispatch_action(Box::new(InstallDevExtension))
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .justify_between()
                            .child(h_flex().child(self.render_search(cx)))
                            .child(
                                h_flex()
                                    .child(
                                        ToggleButton::new("filter-all", "All")
                                            .style(ButtonStyle::Filled)
                                            .size(ButtonSize::Large)
                                            .toggle_state(self.filter == ExtensionFilter::All)
                                            .on_click(cx.listener(|this, _event, cx| {
                                                this.filter = ExtensionFilter::All;
                                                this.filter_extension_entries(cx);
                                            }))
                                            .tooltip(move |cx| {
                                                Tooltip::text("Show all extensions", cx)
                                            })
                                            .first(),
                                    )
                                    .child(
                                        ToggleButton::new("filter-installed", "Installed")
                                            .style(ButtonStyle::Filled)
                                            .size(ButtonSize::Large)
                                            .toggle_state(self.filter == ExtensionFilter::Installed)
                                            .on_click(cx.listener(|this, _event, cx| {
                                                this.filter = ExtensionFilter::Installed;
                                                this.filter_extension_entries(cx);
                                            }))
                                            .tooltip(move |cx| {
                                                Tooltip::text("Show installed extensions", cx)
                                            })
                                            .middle(),
                                    )
                                    .child(
                                        ToggleButton::new("filter-not-installed", "Not Installed")
                                            .style(ButtonStyle::Filled)
                                            .size(ButtonSize::Large)
                                            .toggle_state(
                                                self.filter == ExtensionFilter::NotInstalled,
                                            )
                                            .on_click(cx.listener(|this, _event, cx| {
                                                this.filter = ExtensionFilter::NotInstalled;
                                                this.filter_extension_entries(cx);
                                            }))
                                            .tooltip(move |cx| {
                                                Tooltip::text("Show not installed extensions", cx)
                                            })
                                            .last(),
                                    ),
                            ),
                    ),
            )
            .child(self.render_feature_upsells(cx))
            .child(v_flex().px_4().size_full().overflow_y_hidden().map(|this| {
                let mut count = self.filtered_remote_extension_indices.len();
                if self.filter.include_dev_extensions() {
                    count += self.dev_extension_entries.len();
                }

                if count == 0 {
                    return this.py_4().child(self.render_empty_state(cx));
                }

                let view = cx.view().clone();
                let scroll_handle = self.list.clone();
                this.child(
                    uniform_list(view, "entries", count, Self::render_extensions)
                        .flex_grow()
                        .pb_4()
                        .track_scroll(scroll_handle),
                )
            }))
    }
}

impl EventEmitter<ItemEvent> for ExtensionsPage {}

impl FocusableView for ExtensionsPage {
    fn focus_handle(&self, cx: &AppContext) -> gpui::FocusHandle {
        self.query_editor.read(cx).focus_handle(cx)
    }
}

impl Item for ExtensionsPage {
    type Event = ItemEvent;

    fn tab_content_text(&self, _cx: &WindowContext) -> Option<SharedString> {
        Some("Extensions".into())
    }

    fn telemetry_event_text(&self) -> Option<&'static str> {
        Some("extensions page")
    }

    fn show_toolbar(&self) -> bool {
        false
    }

    fn clone_on_split(
        &self,
        _workspace_id: Option<WorkspaceId>,
        _: &mut ViewContext<Self>,
    ) -> Option<View<Self>> {
        None
    }

    fn to_item_events(event: &Self::Event, mut f: impl FnMut(workspace::item::ItemEvent)) {
        f(*event)
    }
}
