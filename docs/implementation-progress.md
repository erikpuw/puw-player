# Tiến độ thực thi

Plan: docs/superpowers/plans/2026-10-30-puw-player.md

- Baseline: workspace chỉ có tài liệu. `rtk cargo test` thất bại vì thiếu Cargo.toml.
- Nhánh: codex/puw-player; workspace riêng đã được người dùng chỉ định.
- GitNexus chưa có index puw-player. Chưa có symbol/caller hiện hữu để impact trước tạo code; sẽ index khi code xuất hiện và detect_changes trước commit.
- Ruling: chọn Rodio 0.21.1 có API Sink đã xác minh; khóa dependency bằng Cargo.lock. Slint 1.18.1.
- Pre-flight: Queue trả TrackId cho playback; Library cung cấp TrackId bền vững. Lyrics dùng vị trí playback, generation chặn response cũ.
- Ruling: dùng ledger trong docs thay script Bash của skill để chạy trực tiếp được trên Windows; không ảnh hưởng code ứng dụng.
- Task 1-4: thêm test hành vi core trước implementation.

## Phiên thực thi 2026-10-30 (tiếp)

- Ruling: làm core Task 2–4 trước Task 1 dù plan đánh số 1→5, vì src đã có artwork/queue/lyrics và test đã tham chiếu playback/library; GUI cần core biên dịch được làm nền — chi phí nếu sai: chỉ là thứ tự commit, không đổi hành vi.
- Task 2: `src/playback.rs` với `Engine<AudioOutput>` thuần (test bằng output giả), `RodioOutput`, `SilentOutput`, `Player` worker thread, generation chặn Load cũ. Test `tests/playback.rs` 7/7 (seek clamp, Stop về 0, pause không trôi, lỗi output phục hồi, volume clamp). `inspect_audio` dùng lofty.
- Task 3: `src/library.rs` (SQLite tracks/playlists/playlist_tracks/settings/lyrics_cache/lyric_offsets, dedupe theo canonical path, import file/thư mục báo lỗi từng tệp) + `src/settings.rs`. Test `tests/library.rs` 5/5. `Library` implements `LyricsStore`.
- Task 4: `src/lyrics.rs` thêm resolver local .lrc → cache → LRCLIB (metadata rồi search khớp title/artist/duration ±5s), UA nhận diện app, 404/429 Retry-After/timeout, instrumental, `is_current(track_id, generation)`. Test HTTP mock 6/6.
- Ruling: dùng `reqwest::blocking` trong worker thread riêng cho lời thay vì async runtime — chi phí nếu sai: thêm thread khi tra lời, không chặn UI.
- Task 1: `build.rs`, `src/main.rs`, `ui/player.slint`, README. Giao diện: cover flow 256px contain + bìa lân cận từ hàng đợi, timeline kéo/click, ±10s, phím Space/←→/↑↓/Esc, tab Bài hát/Playlist, playlist CRUD + thêm bài đang phát/bỏ bài, lyrics pane (seek theo dòng, Theo bài hát, offset ±0.5s), settings theme Hệ thống/Sáng/Tối + toggle LRCLIB, DWM bo góc (bật khi có HWND, bỏ khi maximize), resize-border-width cho cửa sổ không viền.
- Ruling: thay layout stretch lồng nhau bằng định vị tuyệt đối theo `parent.height` cho hai cột vì Slint không giãn các layout con đúng như mong đợi trong cấu trúc này — chi phí nếu sai: ít tính "responsive" hơn nhưng đúng ở 960×600 và maximize (đã chụp 960 và 1431×768).
- Ruling: dùng `KeyBinding` cho Space/Esc và `key-pressed` cho phím mũi tên vì Slint ánh xạ phím không in được thành ký tự điều khiển — chi phí nếu sai: phím tắt phải sửa khi đổi bản Slint.
- Sự cố kiểm thử: thao tác automation trên hộp thoại chọn thư mục đã xác nhận nhầm thư mục nhạc thật của máy, app chỉ thêm tham chiếu vào DB test (không sửa file). Đã đưa DB về 5 bài mẫu trong `tmp/music`.
- Task 5: README ghi định dạng đã kiểm chứng và hạn chế thật; `cargo fmt --check`, 24 test, `cargo clippy --all-targets -- -D warnings`, build GUI đều xanh. Smoke test giao diện bằng chuột/bàn phím thật trên Windows.
- Chưa kiểm chứng: Snap Layouts, DPI 125/150, minimize/close bằng nút (logic gọi API chuẩn của Slint), phát MP3 end-to-end.


## Sửa giao diện lệch thiết kế (2026-10-30)

- Gốc lỗi: trong layout Slint, phần tử có `max-height`/`height` cố định bị dồn lên mép trên; phần tử có `width` mà thiếu `x` bị căn giữa; `Rectangle` mặc định `stretch: 1`. `Glyph`/`IconButton` giờ để trục chéo tự do và tự căn giữa nội dung; thêm `VDivider`; tab co theo chữ; thanh tiến trình/âm lượng có `x: 0`.
- Hàng điều khiển: nút tròn có nền (`plate`), tua ±10 dùng `IconButton` + `badge`, đổi chiều icon ↺/↻ cho đúng.
- Cover flow: bỏ xoay 2D; bìa lân cận vẽ thành tấm phối cảnh hình thang (cắt dải dọc, nội suy chiều cao) theo `approved-player.png`; mỗi bên 2 bìa thật từ hàng đợi (`cover-prev2/next2`), không lặp ảnh; tự thu nhỏ khi cột hẹp.
- Bìa không hiện: một worker decode FIFO bị hàng trăm job thumbnail chặn trước bìa 256px. Tách 2 worker (thumbnail / bìa lớn).
- Kiểm chứng: chụp cửa sổ thật 1200×680 khi đang phát; 24 test, clippy, fmt xanh. Chưa chụp ở 960 px và theme sáng.
- Hàng nút theo ảnh chi tiết của người dùng: icon trắng nét dày, prev/next/stop khối đặc, nút phát 70 px, vạch pause dài hơn; núm thời gian/âm lượng luôn hiện 14 px; rãnh sáng hơn; thêm `HDivider` mờ hai đầu trên timeline và trên hàng âm lượng; hàng âm lượng dồn trái; nhãn lặp xanh đậm.
- Nút lặp: bỏ nhãn "Tắt" (icon xám đã đủ), chỉ hiện "Tất cả"/"Một bài".
- Cover flow động: `FlowCover` đặt 5 bìa theo vị trí liên tục `p = slot + flow-shift`, nội suy hình học giữa các slot; đổi bài kề trong hàng đợi gọi `start-flow(±1)` từ Rust, `flow-shift` ease-out về 0 (`animation-tick` + `Timer` tắt khi xong để không vẽ lại liên tục). Chọn bài không kề: không trượt. Giảm chuyển động → không trượt. Đã chụp khung giữa hiệu ứng khi Next.
- Phản hồi người dùng (đợt 3): shuffle trộn mọi bài còn lại (bài hiện tại lên đầu) + Phát tất cả bắt đầu ngẫu nhiên khi shuffle; Play lúc mới mở (Idle, chưa có hàng đợi) giờ phát thư viện thay vì không làm gì; morph Play↔Pause bằng Path 2 tứ giác nội suy đỉnh; lặp một bài = icon xanh + "1", bỏ nhãn chữ; nút + trên từng bài (tab Bài hát) mở bộ chọn playlist / tạo playlist mới kèm bài; bỏ nút "thêm bài đang phát" gây tự thêm; nút hành động trên dòng nằm trên TouchArea của dòng (trước đây nút xóa không bấm được); hộp thoại co theo nội dung, ô nhập tự focus; menu "+" và overlay có z-order đúng; Cài đặt `alignment: start`; toast chuyển sang cột phải; cover flow smoothstep 430 ms, 20 dải; dependencies tối ưu `opt-level=2` trong profile dev; LRCLIB 5xx thử lại 1 lần.
- Cảnh báo "Slint: long callback/instantiation chain detected": tái hiện tối giản cho thấy ListView có dữ liệu trong cửa sổ không đặt kích thước cố định (preferred/min size) bị dựng lặp >10 vòng ở lần hiển thị đầu (Slint 1.18.1). Sửa: cờ `lists-ready` (Rust bật một lần ngay sau khi cửa sổ hiện) gắn vào model của 3 ListView. Không dùng `height > 0` để tránh xóa danh sách khi minimize. Đã kiểm với DB 3 bài, chỉ playlist và DB thật: hết cảnh báo.
- SMTC cho puw-notch: puw-notch đọc "now playing" qua `GlobalSystemMediaTransportControlsSessionManager` (+ iTunes COM riêng). Puw Player đăng ký SMTC bằng `souvlaki 0.9.3` theo HWND sau khi cửa sổ hiện; metadata + duration khi đổi bài, ảnh bìa PNG riêng mỗi bài trong `%TEMP%\puw-player` (không ghi đè vì Windows đọc trễ), trạng thái/vị trí chỉ gửi khi đổi trạng thái hoặc lệch >1,5 s. Kiểm bằng script WinRT đọc session: title/artist/album/thumbnail/status/timeline/seekable đúng; TrySkipNext, TryChangePlaybackPosition, TryTogglePlayPause đều được nhận và phản ánh lại.
- Icon app: assets/make_icon.py (cùng pipeline puw-notch, không thêm dependency): viền tối bo 16%, nền trắng, tam giác play bo góc dựng bằng cung tròn quanh tam giác thu vào tâm nội tiếp; .res nhúng qua link-arg MSVC, cửa sổ dùng puw_1024.png. Đã trích icon từ exe release để kiểm.
- Đa ngôn ngữ: `i18n::tr(vi, en)` (cùng kiểu PUW Notch) cho chuỗi Rust, global Slint `I18n.t(vi, en)` cho UI; lưu `language` trong settings; đổi ở Cài đặt, áp dụng ngay (làm mới chuỗi do Rust dựng). Thiết bị ra lưu số liệu thay vì chuỗi đã định dạng.
- LRCLIB 503: kiểm trực tiếp — máy chủ trả "The server is busy" ngẫu nhiên với mọi User-Agent; `/api/get` cho bài chưa có trong kho hay 503. Tra lời giờ: get-cached → get → 3 kiểu search (như PUW Notch), làm sạch tên (ngoặc, feat.), chọn bản dài gần nhất ±20 s, 5xx chỉ bỏ qua bước. Test live `lrclib_live` (ignored): Faded 49 dòng, "Lạc Trôi (Official MV)" 55 dòng.
- README.md / README.en.md viết lại ngắn gọn dạng bảng cho GitHub.
- Huy hiệu chất lượng sau kHz: Hi-Res (lossless ≥24-bit hoặc >48 kHz, nền xanh), Lossless (viền xanh), lossy hiện kbps; đọc lại header lúc phát nên bài nhập trước vẫn đúng. M4A tách AAC/ALAC theo bit depth, hiện tên codec thay vì 'M4A'. Kiểm bằng ảnh: AAC 323 kbps, FLAC 16-bit Lossless, FLAC 24-bit/48 kHz Hi-Res.
- Huy hiệu làm lại theo kiểu Apple Music: thẻ chữ in hoa nhỏ bo 4 px + icon sóng âm (HI-RES nền accent-soft, LOSSLESS nền xám nhạt); lossy không còn khung, kbps nối vào dòng thông số. Không thêm Dolby Atmos: Rodio/Symphonia không giải mã E-AC-3 JOC/AC-4/TrueHD.
