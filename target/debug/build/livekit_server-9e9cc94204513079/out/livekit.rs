/// composite using a web browser
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RoomCompositeEgressRequest {
    /// required
    #[prost(string, tag="1")]
    pub room_name: ::prost::alloc::string::String,
    /// (optional)
    #[prost(string, tag="2")]
    pub layout: ::prost::alloc::string::String,
    /// (default false)
    #[prost(bool, tag="3")]
    pub audio_only: bool,
    /// (default false)
    #[prost(bool, tag="4")]
    pub video_only: bool,
    /// (default <https://recorder.livekit.io>)
    #[prost(string, tag="5")]
    pub custom_base_url: ::prost::alloc::string::String,
    /// required
    #[prost(oneof="room_composite_egress_request::Output", tags="6, 7, 10")]
    pub output: ::core::option::Option<room_composite_egress_request::Output>,
    #[prost(oneof="room_composite_egress_request::Options", tags="8, 9")]
    pub options: ::core::option::Option<room_composite_egress_request::Options>,
}
/// Nested message and enum types in `RoomCompositeEgressRequest`.
pub mod room_composite_egress_request {
    /// required
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Output {
        #[prost(message, tag="6")]
        File(super::EncodedFileOutput),
        #[prost(message, tag="7")]
        Stream(super::StreamOutput),
        #[prost(message, tag="10")]
        Segments(super::SegmentedFileOutput),
    }
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Options {
        /// (default H264_720P_30)
        #[prost(enumeration="super::EncodingOptionsPreset", tag="8")]
        Preset(i32),
        /// (optional)
        #[prost(message, tag="9")]
        Advanced(super::EncodingOptions),
    }
}
/// containerize up to one audio and one video track
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TrackCompositeEgressRequest {
    /// required
    #[prost(string, tag="1")]
    pub room_name: ::prost::alloc::string::String,
    /// (optional)
    #[prost(string, tag="2")]
    pub audio_track_id: ::prost::alloc::string::String,
    /// (optional)
    #[prost(string, tag="3")]
    pub video_track_id: ::prost::alloc::string::String,
    /// required
    #[prost(oneof="track_composite_egress_request::Output", tags="4, 5, 8")]
    pub output: ::core::option::Option<track_composite_egress_request::Output>,
    #[prost(oneof="track_composite_egress_request::Options", tags="6, 7")]
    pub options: ::core::option::Option<track_composite_egress_request::Options>,
}
/// Nested message and enum types in `TrackCompositeEgressRequest`.
pub mod track_composite_egress_request {
    /// required
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Output {
        #[prost(message, tag="4")]
        File(super::EncodedFileOutput),
        #[prost(message, tag="5")]
        Stream(super::StreamOutput),
        #[prost(message, tag="8")]
        Segments(super::SegmentedFileOutput),
    }
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Options {
        /// (default H264_720P_30)
        #[prost(enumeration="super::EncodingOptionsPreset", tag="6")]
        Preset(i32),
        /// (optional)
        #[prost(message, tag="7")]
        Advanced(super::EncodingOptions),
    }
}
/// record tracks individually, without transcoding
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TrackEgressRequest {
    /// required
    #[prost(string, tag="1")]
    pub room_name: ::prost::alloc::string::String,
    /// required
    #[prost(string, tag="2")]
    pub track_id: ::prost::alloc::string::String,
    /// required
    #[prost(oneof="track_egress_request::Output", tags="3, 4")]
    pub output: ::core::option::Option<track_egress_request::Output>,
}
/// Nested message and enum types in `TrackEgressRequest`.
pub mod track_egress_request {
    /// required
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Output {
        #[prost(message, tag="3")]
        File(super::DirectFileOutput),
        #[prost(string, tag="4")]
        WebsocketUrl(::prost::alloc::string::String),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EncodedFileOutput {
    /// (optional)
    #[prost(enumeration="EncodedFileType", tag="1")]
    pub file_type: i32,
    /// (optional)
    #[prost(string, tag="2")]
    pub filepath: ::prost::alloc::string::String,
    /// required
    #[prost(oneof="encoded_file_output::Output", tags="3, 4, 5")]
    pub output: ::core::option::Option<encoded_file_output::Output>,
}
/// Nested message and enum types in `EncodedFileOutput`.
pub mod encoded_file_output {
    /// required
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Output {
        #[prost(message, tag="3")]
        S3(super::S3Upload),
        #[prost(message, tag="4")]
        Gcp(super::GcpUpload),
        #[prost(message, tag="5")]
        Azure(super::AzureBlobUpload),
    }
}
/// Used to generate HLS segments or other kind of segmented output
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SegmentedFileOutput {
    /// (optional)
    #[prost(enumeration="SegmentedFileProtocol", tag="1")]
    pub protocol: i32,
    /// (optional)
    #[prost(string, tag="2")]
    pub filename_prefix: ::prost::alloc::string::String,
    /// (optional)
    #[prost(string, tag="3")]
    pub playlist_name: ::prost::alloc::string::String,
    /// (optional)
    #[prost(uint32, tag="4")]
    pub segment_duration: u32,
    /// required
    #[prost(oneof="segmented_file_output::Output", tags="5, 6, 7")]
    pub output: ::core::option::Option<segmented_file_output::Output>,
}
/// Nested message and enum types in `SegmentedFileOutput`.
pub mod segmented_file_output {
    /// required
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Output {
        #[prost(message, tag="5")]
        S3(super::S3Upload),
        #[prost(message, tag="6")]
        Gcp(super::GcpUpload),
        #[prost(message, tag="7")]
        Azure(super::AzureBlobUpload),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DirectFileOutput {
    /// (optional)
    #[prost(string, tag="1")]
    pub filepath: ::prost::alloc::string::String,
    /// required
    #[prost(oneof="direct_file_output::Output", tags="2, 3, 4")]
    pub output: ::core::option::Option<direct_file_output::Output>,
}
/// Nested message and enum types in `DirectFileOutput`.
pub mod direct_file_output {
    /// required
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Output {
        #[prost(message, tag="2")]
        S3(super::S3Upload),
        #[prost(message, tag="3")]
        Gcp(super::GcpUpload),
        #[prost(message, tag="4")]
        Azure(super::AzureBlobUpload),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct S3Upload {
    #[prost(string, tag="1")]
    pub access_key: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub secret: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub region: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub endpoint: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub bucket: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GcpUpload {
    #[prost(bytes="vec", tag="1")]
    pub credentials: ::prost::alloc::vec::Vec<u8>,
    #[prost(string, tag="2")]
    pub bucket: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AzureBlobUpload {
    #[prost(string, tag="1")]
    pub account_name: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub account_key: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub container_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamOutput {
    /// required
    #[prost(enumeration="StreamProtocol", tag="1")]
    pub protocol: i32,
    /// required
    #[prost(string, repeated, tag="2")]
    pub urls: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EncodingOptions {
    /// (default 1920)
    #[prost(int32, tag="1")]
    pub width: i32,
    /// (default 1080)
    #[prost(int32, tag="2")]
    pub height: i32,
    /// (default 24)
    #[prost(int32, tag="3")]
    pub depth: i32,
    /// (default 30)
    #[prost(int32, tag="4")]
    pub framerate: i32,
    /// (default OPUS)
    #[prost(enumeration="AudioCodec", tag="5")]
    pub audio_codec: i32,
    /// (default 128)
    #[prost(int32, tag="6")]
    pub audio_bitrate: i32,
    /// (default 44100)
    #[prost(int32, tag="7")]
    pub audio_frequency: i32,
    /// (default H264_MAIN)
    #[prost(enumeration="VideoCodec", tag="8")]
    pub video_codec: i32,
    /// (default 4500)
    #[prost(int32, tag="9")]
    pub video_bitrate: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateLayoutRequest {
    #[prost(string, tag="1")]
    pub egress_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub layout: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateStreamRequest {
    #[prost(string, tag="1")]
    pub egress_id: ::prost::alloc::string::String,
    #[prost(string, repeated, tag="2")]
    pub add_output_urls: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag="3")]
    pub remove_output_urls: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListEgressRequest {
    /// (optional, used to filter results)
    #[prost(string, tag="1")]
    pub room_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListEgressResponse {
    #[prost(message, repeated, tag="1")]
    pub items: ::prost::alloc::vec::Vec<EgressInfo>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StopEgressRequest {
    #[prost(string, tag="1")]
    pub egress_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EgressInfo {
    #[prost(string, tag="1")]
    pub egress_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub room_id: ::prost::alloc::string::String,
    #[prost(string, tag="13")]
    pub room_name: ::prost::alloc::string::String,
    #[prost(enumeration="EgressStatus", tag="3")]
    pub status: i32,
    #[prost(int64, tag="10")]
    pub started_at: i64,
    #[prost(int64, tag="11")]
    pub ended_at: i64,
    #[prost(string, tag="9")]
    pub error: ::prost::alloc::string::String,
    #[prost(oneof="egress_info::Request", tags="4, 5, 6")]
    pub request: ::core::option::Option<egress_info::Request>,
    #[prost(oneof="egress_info::Result", tags="7, 8, 12")]
    pub result: ::core::option::Option<egress_info::Result>,
}
/// Nested message and enum types in `EgressInfo`.
pub mod egress_info {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Request {
        #[prost(message, tag="4")]
        RoomComposite(super::RoomCompositeEgressRequest),
        #[prost(message, tag="5")]
        TrackComposite(super::TrackCompositeEgressRequest),
        #[prost(message, tag="6")]
        Track(super::TrackEgressRequest),
    }
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Result {
        #[prost(message, tag="7")]
        Stream(super::StreamInfoList),
        #[prost(message, tag="8")]
        File(super::FileInfo),
        #[prost(message, tag="12")]
        Segments(super::SegmentsInfo),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamInfoList {
    #[prost(message, repeated, tag="1")]
    pub info: ::prost::alloc::vec::Vec<StreamInfo>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamInfo {
    #[prost(string, tag="1")]
    pub url: ::prost::alloc::string::String,
    #[prost(int64, tag="2")]
    pub started_at: i64,
    #[prost(int64, tag="3")]
    pub ended_at: i64,
    #[prost(int64, tag="4")]
    pub duration: i64,
    #[prost(enumeration="stream_info::Status", tag="5")]
    pub status: i32,
}
/// Nested message and enum types in `StreamInfo`.
pub mod stream_info {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Status {
        Active = 0,
        Finished = 1,
        Failed = 2,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FileInfo {
    #[prost(string, tag="1")]
    pub filename: ::prost::alloc::string::String,
    #[prost(int64, tag="6")]
    pub duration: i64,
    #[prost(int64, tag="4")]
    pub size: i64,
    #[prost(string, tag="5")]
    pub location: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SegmentsInfo {
    #[prost(string, tag="1")]
    pub playlist_name: ::prost::alloc::string::String,
    #[prost(int64, tag="2")]
    pub duration: i64,
    #[prost(int64, tag="3")]
    pub size: i64,
    #[prost(string, tag="4")]
    pub playlist_location: ::prost::alloc::string::String,
    #[prost(int64, tag="5")]
    pub segment_count: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum EncodedFileType {
    /// file type chosen based on codecs
    DefaultFiletype = 0,
    Mp4 = 1,
    Ogg = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum StreamProtocol {
    /// protocol chosen based on urls
    DefaultProtocol = 0,
    Rtmp = 1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SegmentedFileProtocol {
    DefaultSegmentedFileProtocol = 0,
    HlsProtocol = 1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum AudioCodec {
    DefaultAc = 0,
    Opus = 1,
    Aac = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum VideoCodec {
    DefaultVc = 0,
    H264Baseline = 1,
    H264Main = 2,
    H264High = 3,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum EncodingOptionsPreset {
    ///  1280x720, 30fps, 3000kpbs, H.264_MAIN / OPUS
    H264720p30 = 0,
    ///  1280x720, 60fps, 4500kbps, H.264_MAIN / OPUS
    H264720p60 = 1,
    /// 1920x1080, 30fps, 4500kbps, H.264_MAIN / OPUS
    H2641080p30 = 2,
    /// 1920x1080, 60fps, 6000kbps, H.264_MAIN / OPUS
    H2641080p60 = 3,
    ///  720x1280, 30fps, 3000kpbs, H.264_MAIN / OPUS
    PortraitH264720p30 = 4,
    ///  720x1280, 60fps, 4500kbps, H.264_MAIN / OPUS
    PortraitH264720p60 = 5,
    /// 1080x1920, 30fps, 4500kbps, H.264_MAIN / OPUS
    PortraitH2641080p30 = 6,
    /// 1080x1920, 60fps, 6000kbps, H.264_MAIN / OPUS
    PortraitH2641080p60 = 7,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum EgressStatus {
    EgressStarting = 0,
    EgressActive = 1,
    EgressEnding = 2,
    EgressComplete = 3,
    EgressFailed = 4,
    EgressAborted = 5,
    EgressLimitReached = 6,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Room {
    #[prost(string, tag="1")]
    pub sid: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub name: ::prost::alloc::string::String,
    #[prost(uint32, tag="3")]
    pub empty_timeout: u32,
    #[prost(uint32, tag="4")]
    pub max_participants: u32,
    #[prost(int64, tag="5")]
    pub creation_time: i64,
    #[prost(string, tag="6")]
    pub turn_password: ::prost::alloc::string::String,
    #[prost(message, repeated, tag="7")]
    pub enabled_codecs: ::prost::alloc::vec::Vec<Codec>,
    #[prost(string, tag="8")]
    pub metadata: ::prost::alloc::string::String,
    #[prost(uint32, tag="9")]
    pub num_participants: u32,
    #[prost(bool, tag="10")]
    pub active_recording: bool,
}
/// room info that should not be returned to clients
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RoomInternal {
    #[prost(message, optional, tag="1")]
    pub track_egress: ::core::option::Option<AutoTrackEgress>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AutoTrackEgress {
    #[prost(string, tag="1")]
    pub file_prefix: ::prost::alloc::string::String,
    #[prost(oneof="auto_track_egress::Output", tags="2, 3, 4")]
    pub output: ::core::option::Option<auto_track_egress::Output>,
}
/// Nested message and enum types in `AutoTrackEgress`.
pub mod auto_track_egress {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Output {
        #[prost(message, tag="2")]
        S3(super::S3Upload),
        #[prost(message, tag="3")]
        Gcp(super::GcpUpload),
        #[prost(message, tag="4")]
        Azure(super::AzureBlobUpload),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Codec {
    #[prost(string, tag="1")]
    pub mime: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub fmtp_line: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ParticipantPermission {
    /// allow participant to subscribe to other tracks in the room
    #[prost(bool, tag="1")]
    pub can_subscribe: bool,
    /// allow participant to publish new tracks to room
    #[prost(bool, tag="2")]
    pub can_publish: bool,
    /// allow participant to publish data
    #[prost(bool, tag="3")]
    pub can_publish_data: bool,
    /// indicates that it's hidden to others
    #[prost(bool, tag="7")]
    pub hidden: bool,
    /// indicates it's a recorder instance
    #[prost(bool, tag="8")]
    pub recorder: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ParticipantInfo {
    #[prost(string, tag="1")]
    pub sid: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub identity: ::prost::alloc::string::String,
    #[prost(enumeration="participant_info::State", tag="3")]
    pub state: i32,
    #[prost(message, repeated, tag="4")]
    pub tracks: ::prost::alloc::vec::Vec<TrackInfo>,
    #[prost(string, tag="5")]
    pub metadata: ::prost::alloc::string::String,
    /// timestamp when participant joined room, in seconds
    #[prost(int64, tag="6")]
    pub joined_at: i64,
    #[prost(string, tag="9")]
    pub name: ::prost::alloc::string::String,
    #[prost(uint32, tag="10")]
    pub version: u32,
    #[prost(message, optional, tag="11")]
    pub permission: ::core::option::Option<ParticipantPermission>,
    #[prost(string, tag="12")]
    pub region: ::prost::alloc::string::String,
    /// indicates the participant has an active publisher connection
    /// and can publish to the server
    #[prost(bool, tag="13")]
    pub is_publisher: bool,
}
/// Nested message and enum types in `ParticipantInfo`.
pub mod participant_info {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum State {
        /// websocket' connected, but not offered yet
        Joining = 0,
        /// server received client offer
        Joined = 1,
        /// ICE connectivity established
        Active = 2,
        /// WS disconnected
        Disconnected = 3,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SimulcastCodecInfo {
    #[prost(string, tag="1")]
    pub mime_type: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub mid: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub cid: ::prost::alloc::string::String,
    #[prost(message, repeated, tag="4")]
    pub layers: ::prost::alloc::vec::Vec<VideoLayer>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TrackInfo {
    #[prost(string, tag="1")]
    pub sid: ::prost::alloc::string::String,
    #[prost(enumeration="TrackType", tag="2")]
    pub r#type: i32,
    #[prost(string, tag="3")]
    pub name: ::prost::alloc::string::String,
    #[prost(bool, tag="4")]
    pub muted: bool,
    /// original width of video (unset for audio)
    /// clients may receive a lower resolution version with simulcast
    #[prost(uint32, tag="5")]
    pub width: u32,
    /// original height of video (unset for audio)
    #[prost(uint32, tag="6")]
    pub height: u32,
    /// true if track is simulcasted
    #[prost(bool, tag="7")]
    pub simulcast: bool,
    /// true if DTX (Discontinuous Transmission) is disabled for audio
    #[prost(bool, tag="8")]
    pub disable_dtx: bool,
    /// source of media
    #[prost(enumeration="TrackSource", tag="9")]
    pub source: i32,
    #[prost(message, repeated, tag="10")]
    pub layers: ::prost::alloc::vec::Vec<VideoLayer>,
    /// mime type of codec
    #[prost(string, tag="11")]
    pub mime_type: ::prost::alloc::string::String,
    #[prost(string, tag="12")]
    pub mid: ::prost::alloc::string::String,
    #[prost(message, repeated, tag="13")]
    pub codecs: ::prost::alloc::vec::Vec<SimulcastCodecInfo>,
}
/// provide information about available spatial layers
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VideoLayer {
    /// for tracks with a single layer, this should be HIGH
    #[prost(enumeration="VideoQuality", tag="1")]
    pub quality: i32,
    #[prost(uint32, tag="2")]
    pub width: u32,
    #[prost(uint32, tag="3")]
    pub height: u32,
    /// target bitrate, server will measure actual
    #[prost(uint32, tag="4")]
    pub bitrate: u32,
    #[prost(uint32, tag="5")]
    pub ssrc: u32,
}
/// new DataPacket API
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DataPacket {
    #[prost(enumeration="data_packet::Kind", tag="1")]
    pub kind: i32,
    #[prost(oneof="data_packet::Value", tags="2, 3")]
    pub value: ::core::option::Option<data_packet::Value>,
}
/// Nested message and enum types in `DataPacket`.
pub mod data_packet {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Kind {
        Reliable = 0,
        Lossy = 1,
    }
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Value {
        #[prost(message, tag="2")]
        User(super::UserPacket),
        #[prost(message, tag="3")]
        Speaker(super::ActiveSpeakerUpdate),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ActiveSpeakerUpdate {
    #[prost(message, repeated, tag="1")]
    pub speakers: ::prost::alloc::vec::Vec<SpeakerInfo>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SpeakerInfo {
    #[prost(string, tag="1")]
    pub sid: ::prost::alloc::string::String,
    /// audio level, 0-1.0, 1 is loudest
    #[prost(float, tag="2")]
    pub level: f32,
    /// true if speaker is currently active
    #[prost(bool, tag="3")]
    pub active: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UserPacket {
    /// participant ID of user that sent the message
    #[prost(string, tag="1")]
    pub participant_sid: ::prost::alloc::string::String,
    /// user defined payload
    #[prost(bytes="vec", tag="2")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
    /// the ID of the participants who will receive the message (the message will be sent to all the people in the room if this variable is empty)
    #[prost(string, repeated, tag="3")]
    pub destination_sids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ParticipantTracks {
    /// participant ID of participant to whom the tracks belong
    #[prost(string, tag="1")]
    pub participant_sid: ::prost::alloc::string::String,
    #[prost(string, repeated, tag="2")]
    pub track_sids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// details about the server
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerInfo {
    #[prost(enumeration="server_info::Edition", tag="1")]
    pub edition: i32,
    #[prost(string, tag="2")]
    pub version: ::prost::alloc::string::String,
    #[prost(int32, tag="3")]
    pub protocol: i32,
    #[prost(string, tag="4")]
    pub region: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub node_id: ::prost::alloc::string::String,
    /// additional debugging information. sent only if server is in development mode
    #[prost(string, tag="6")]
    pub debug_info: ::prost::alloc::string::String,
}
/// Nested message and enum types in `ServerInfo`.
pub mod server_info {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Edition {
        Standard = 0,
        Cloud = 1,
    }
}
/// details about the client
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ClientInfo {
    #[prost(enumeration="client_info::Sdk", tag="1")]
    pub sdk: i32,
    #[prost(string, tag="2")]
    pub version: ::prost::alloc::string::String,
    #[prost(int32, tag="3")]
    pub protocol: i32,
    #[prost(string, tag="4")]
    pub os: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub os_version: ::prost::alloc::string::String,
    #[prost(string, tag="6")]
    pub device_model: ::prost::alloc::string::String,
    #[prost(string, tag="7")]
    pub browser: ::prost::alloc::string::String,
    #[prost(string, tag="8")]
    pub browser_version: ::prost::alloc::string::String,
    #[prost(string, tag="9")]
    pub address: ::prost::alloc::string::String,
    /// wifi, wired, cellular, vpn, empty if not known
    #[prost(string, tag="10")]
    pub network: ::prost::alloc::string::String,
}
/// Nested message and enum types in `ClientInfo`.
pub mod client_info {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Sdk {
        Unknown = 0,
        Js = 1,
        Swift = 2,
        Android = 3,
        Flutter = 4,
        Go = 5,
        Unity = 6,
    }
}
/// server provided client configuration
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ClientConfiguration {
    #[prost(message, optional, tag="1")]
    pub video: ::core::option::Option<VideoConfiguration>,
    #[prost(message, optional, tag="2")]
    pub screen: ::core::option::Option<VideoConfiguration>,
    #[prost(enumeration="ClientConfigSetting", tag="3")]
    pub resume_connection: i32,
    #[prost(message, optional, tag="4")]
    pub disabled_codecs: ::core::option::Option<DisabledCodecs>,
    #[prost(enumeration="ClientConfigSetting", tag="5")]
    pub force_relay: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VideoConfiguration {
    #[prost(enumeration="ClientConfigSetting", tag="1")]
    pub hardware_encoder: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DisabledCodecs {
    #[prost(message, repeated, tag="1")]
    pub codecs: ::prost::alloc::vec::Vec<Codec>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RtpStats {
    #[prost(message, optional, tag="1")]
    pub start_time: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag="2")]
    pub end_time: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(double, tag="3")]
    pub duration: f64,
    #[prost(uint32, tag="4")]
    pub packets: u32,
    #[prost(double, tag="5")]
    pub packet_rate: f64,
    #[prost(uint64, tag="6")]
    pub bytes: u64,
    #[prost(uint64, tag="39")]
    pub header_bytes: u64,
    #[prost(double, tag="7")]
    pub bitrate: f64,
    #[prost(uint32, tag="8")]
    pub packets_lost: u32,
    #[prost(double, tag="9")]
    pub packet_loss_rate: f64,
    #[prost(float, tag="10")]
    pub packet_loss_percentage: f32,
    #[prost(uint32, tag="11")]
    pub packets_duplicate: u32,
    #[prost(double, tag="12")]
    pub packet_duplicate_rate: f64,
    #[prost(uint64, tag="13")]
    pub bytes_duplicate: u64,
    #[prost(uint64, tag="40")]
    pub header_bytes_duplicate: u64,
    #[prost(double, tag="14")]
    pub bitrate_duplicate: f64,
    #[prost(uint32, tag="15")]
    pub packets_padding: u32,
    #[prost(double, tag="16")]
    pub packet_padding_rate: f64,
    #[prost(uint64, tag="17")]
    pub bytes_padding: u64,
    #[prost(uint64, tag="41")]
    pub header_bytes_padding: u64,
    #[prost(double, tag="18")]
    pub bitrate_padding: f64,
    #[prost(uint32, tag="19")]
    pub packets_out_of_order: u32,
    #[prost(uint32, tag="20")]
    pub frames: u32,
    #[prost(double, tag="21")]
    pub frame_rate: f64,
    #[prost(double, tag="22")]
    pub jitter_current: f64,
    #[prost(double, tag="23")]
    pub jitter_max: f64,
    #[prost(map="int32, uint32", tag="24")]
    pub gap_histogram: ::std::collections::HashMap<i32, u32>,
    #[prost(uint32, tag="25")]
    pub nacks: u32,
    #[prost(uint32, tag="37")]
    pub nack_acks: u32,
    #[prost(uint32, tag="26")]
    pub nack_misses: u32,
    #[prost(uint32, tag="38")]
    pub nack_repeated: u32,
    #[prost(uint32, tag="27")]
    pub plis: u32,
    #[prost(message, optional, tag="28")]
    pub last_pli: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(uint32, tag="29")]
    pub firs: u32,
    #[prost(message, optional, tag="30")]
    pub last_fir: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(uint32, tag="31")]
    pub rtt_current: u32,
    #[prost(uint32, tag="32")]
    pub rtt_max: u32,
    #[prost(uint32, tag="33")]
    pub key_frames: u32,
    #[prost(message, optional, tag="34")]
    pub last_key_frame: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(uint32, tag="35")]
    pub layer_lock_plis: u32,
    #[prost(message, optional, tag="36")]
    pub last_layer_lock_pli: ::core::option::Option<::prost_types::Timestamp>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TimedVersion {
    #[prost(int64, tag="1")]
    pub unix_micro: i64,
    #[prost(int32, tag="2")]
    pub ticks: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum TrackType {
    Audio = 0,
    Video = 1,
    Data = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum TrackSource {
    Unknown = 0,
    Camera = 1,
    Microphone = 2,
    ScreenShare = 3,
    ScreenShareAudio = 4,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum VideoQuality {
    Low = 0,
    Medium = 1,
    High = 2,
    Off = 3,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ConnectionQuality {
    Poor = 0,
    Good = 1,
    Excellent = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ClientConfigSetting {
    Unset = 0,
    Disabled = 1,
    Enabled = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum DisconnectReason {
    UnknownReason = 0,
    ClientInitiated = 1,
    DuplicateIdentity = 2,
    ServerShutdown = 3,
    ParticipantRemoved = 4,
    RoomDeleted = 5,
    StateMismatch = 6,
    JoinFailure = 7,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateRoomRequest {
    /// name of the room
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// number of seconds to keep the room open if no one joins
    #[prost(uint32, tag="2")]
    pub empty_timeout: u32,
    /// limit number of participants that can be in a room
    #[prost(uint32, tag="3")]
    pub max_participants: u32,
    /// override the node room is allocated to, for debugging
    #[prost(string, tag="4")]
    pub node_id: ::prost::alloc::string::String,
    /// metadata of room
    #[prost(string, tag="5")]
    pub metadata: ::prost::alloc::string::String,
    /// egress
    #[prost(message, optional, tag="6")]
    pub egress: ::core::option::Option<RoomEgress>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RoomEgress {
    #[prost(message, optional, tag="1")]
    pub room: ::core::option::Option<RoomCompositeEgressRequest>,
    #[prost(message, optional, tag="2")]
    pub tracks: ::core::option::Option<AutoTrackEgress>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListRoomsRequest {
    /// when set, will only return rooms with name match
    #[prost(string, repeated, tag="1")]
    pub names: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListRoomsResponse {
    #[prost(message, repeated, tag="1")]
    pub rooms: ::prost::alloc::vec::Vec<Room>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteRoomRequest {
    /// name of the room
    #[prost(string, tag="1")]
    pub room: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteRoomResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListParticipantsRequest {
    /// name of the room
    #[prost(string, tag="1")]
    pub room: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListParticipantsResponse {
    #[prost(message, repeated, tag="1")]
    pub participants: ::prost::alloc::vec::Vec<ParticipantInfo>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RoomParticipantIdentity {
    /// name of the room
    #[prost(string, tag="1")]
    pub room: ::prost::alloc::string::String,
    /// identity of the participant
    #[prost(string, tag="2")]
    pub identity: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveParticipantResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MuteRoomTrackRequest {
    /// name of the room
    #[prost(string, tag="1")]
    pub room: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub identity: ::prost::alloc::string::String,
    /// sid of the track to mute
    #[prost(string, tag="3")]
    pub track_sid: ::prost::alloc::string::String,
    /// set to true to mute, false to unmute
    #[prost(bool, tag="4")]
    pub muted: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MuteRoomTrackResponse {
    #[prost(message, optional, tag="1")]
    pub track: ::core::option::Option<TrackInfo>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateParticipantRequest {
    #[prost(string, tag="1")]
    pub room: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub identity: ::prost::alloc::string::String,
    /// metadata to update. skipping updates if left empty
    #[prost(string, tag="3")]
    pub metadata: ::prost::alloc::string::String,
    /// set to update the participant's permissions
    #[prost(message, optional, tag="4")]
    pub permission: ::core::option::Option<ParticipantPermission>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateSubscriptionsRequest {
    #[prost(string, tag="1")]
    pub room: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub identity: ::prost::alloc::string::String,
    /// list of sids of tracks
    #[prost(string, repeated, tag="3")]
    pub track_sids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    /// set to true to subscribe, false to unsubscribe from tracks
    #[prost(bool, tag="4")]
    pub subscribe: bool,
    /// list of participants and their tracks
    #[prost(message, repeated, tag="5")]
    pub participant_tracks: ::prost::alloc::vec::Vec<ParticipantTracks>,
}
/// empty for now
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateSubscriptionsResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SendDataRequest {
    #[prost(string, tag="1")]
    pub room: ::prost::alloc::string::String,
    #[prost(bytes="vec", tag="2")]
    pub data: ::prost::alloc::vec::Vec<u8>,
    #[prost(enumeration="data_packet::Kind", tag="3")]
    pub kind: i32,
    #[prost(string, repeated, tag="4")]
    pub destination_sids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
///
#[allow(clippy::empty_docs)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SendDataResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateRoomMetadataRequest {
    #[prost(string, tag="1")]
    pub room: ::prost::alloc::string::String,
    /// metadata to update. skipping updates if left empty
    #[prost(string, tag="2")]
    pub metadata: ::prost::alloc::string::String,
}
