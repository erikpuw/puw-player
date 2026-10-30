# Puw Player Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Xây app Windows nghe nhạc offline theo thiết kế đã duyệt, có playlist, Cover Flow và lời đồng bộ tùy bật.

**Architecture:** Rust giữ trạng thái phát và hàng đợi; Slint thể hiện UI. Audio và quét thư mục không chặn luồng UI. SQLite lưu thư viện, playlist, cài đặt và cache lời.

**Tech Stack:** Rust, Slint, Rodio/Symphonia; chọn phiên bản ổn định tương thích sau thử nghiệm tối thiểu. Không thêm web frontend.

**Spec:** `../specs/2026-10-30-puw-player-design.md`

## Global Constraints

- Windows, mặc định 1200×680; bìa tối đa 256×256, contain.
- Bo ngoài 12 px, bìa 6 px; player tròn; maximize không bo ngoài.
- Không thay đổi/xóa file nhạc khi sửa playlist. Không giả trạng thái phát hoặc chất lượng âm thanh.
- Test trước code; graph impact trước sửa symbol hiện hữu; detect_changes trước commit khi có index.
- Không build npm; dự án Rust dùng Cargo. Các yêu cầu của spec áp dụng toàn bộ.

## Review Focus

- Bài mới được chọn trong lúc bài cũ còn tải: response cũ không được ghi đè bài mới (task 2, 4).
- Playlist rỗng, một bài, file mất: không panic, không vòng lặp bỏ bài vô hạn (task 2, 3).
- Seek, repeat, pause: vị trí phát và dòng lời không trôi khỏi nhau (task 2, 4).
- Bìa ngang/dọc và DPI lớn: không méo ảnh, không mất nút hay vùng resize (task 1, 5).
- Mất mạng hoặc thiết bị âm thanh: có lỗi phục hồi được, không mất thư viện (task 2, 4).

## Baseline

Workspace ban đầu trống. `rtk cargo test` báo chưa có Cargo.toml; chưa có test/code để chạy hoặc phân tích caller. Đây không phải test pass.

## Task 1: Cửa sổ và UI thật

Files: Cargo.toml, build.rs, src/main.rs, ui/player.slint, README.md.

- [ ] Xác minh phiên bản Slint/Rodio và hỗ trợ Windows; thử Slint Cover Flow/clip/opacity và cửa sổ không khung trước khi chốt renderer. Nếu Slint không thể làm đúng, báo hạn chế trước đổi stack.
- [ ] Viết test tính fit bìa: ảnh 400×200 vào 256×256 cho 256×128; 200×400 cho 128×256; kích thước không hợp lệ bị từ chối.
- [ ] Chạy test cho thấy chưa có hàm `fit_cover(width: u32, height: u32, max: u32) -> Option<(u32, u32)>`, rồi triển khai tại src/artwork.rs và chạy lại.
- [ ] Dựng layout theo spec và hình duyệt, gồm trạng thái trống; icon cùng họ và có accessible label. Không biến ảnh screenshot thành UI.
- [ ] Kiểm tra kéo/resize/min/max/close, 1200×680 và 960×600, DPI 100/125/150%; lưu ảnh đối chiếu.

## Task 2: Phát âm thanh và hàng đợi

Files: src/playback.rs, src/queue.rs, tests/playback.rs.

Interfaces: `RepeatMode::{Off, All, One}`; `Queue` giữ thứ tự phát và lịch sử; `PlaybackCommand` gồm Load, Play, Pause, Stop, Seek, Volume; snapshot trả track ID, trạng thái, vị trí, duration và lỗi.

- [ ] Viết test cuối hàng đợi Off dừng, All quay đầu, One giữ bài; hàng đợi rỗng không panic; shuffle một lượt không lặp và Previous đi theo lịch sử.
- [ ] Chạy test thất bại trước khi triển khai logic; thực thi decoder/output ngoài UI, dùng generation ID tránh hoàn tất Load cũ ghi đè bài mới.
- [ ] Kiểm thử seek clamp 0..duration, Stop về 0, pause không tiến vị trí, mất output có lỗi phục hồi được.
- [ ] Phát file mẫu hợp pháp cho từng codec/container, kiểm tra duration/seek; chỉ ghi định dạng đã chứng minh trong README.

## Task 3: Thư viện, playlist và cài đặt

Files: src/library.rs, src/settings.rs, tests/library.rs.

Interfaces: `TrackId`, `PlaylistId`; Library sở hữu SQLite, đường dẫn nhạc, metadata và thứ tự playlist. UI chọn playlist không được tự thay Queue; chỉ lệnh phát mới tạo Queue.

- [ ] Viết test nhập trùng không nhân bản, Unicode path đọc được, xóa playlist không xóa file nhạc và khởi động lại giữ thứ tự.
- [ ] Chạy test thất bại, rồi triển khai import file/folder/drop ngoài UI, đọc cover/metadata với thư viện đã xác minh.
- [ ] Thêm tạo/đổi tên/xóa playlist và thêm/bỏ bài; lỗi file mất hiển thị tại bài, không vòng lặp skip vô hạn.
- [ ] Nối UI tab, trang cài đặt và thiết bị/theme; kiểm tra mở cài đặt không ngắt playback.

## Task 4: Lyrics offline và LRCLIB

Files: src/lyrics.rs, tests/lyrics.rs.

Interfaces: `LyricLine { time_ms: u64, text: String }`; `parse_lrc(text: &str) -> Vec<LyricLine>`; `active_line(lines: &[LyricLine], position_ms: u64, offset_ms: i64) -> Option<usize>`. Nguồn lời trả kèm track ID/generation để chống stale response.

- [ ] Test nhiều timestamp trên một dòng, dòng hỏng, timestamp trùng, offset âm, seek lùi và thời điểm trước câu đầu.
- [ ] Chạy test thất bại, triển khai parser và tra dòng theo clock audio; nối toggle Lyrics và nút Theo bài hát.
- [ ] Thêm local .lrc/cache/LRCLIB theo thứ tự spec; tải online có cài đặt rõ và nhận diện client.
- [ ] Dùng HTTP mock để test 404, 429 Retry-After, timeout, lời thường, instrumental, đổi track khi request còn chạy; test offline cache không gửi mạng.

## Task 5: Hoàn thiện và bàn giao

Files: README.md, ui/player.slint, các module cần sửa theo lỗi kiểm thử.

- [ ] Làm morph icon và Cover Flow, tôn trọng reduced motion, không chạy animation liên tục khi không cần.
- [ ] Kiểm thử cả tab, lyrics, playlist, empty/error/loading, bàn phím, DPI và theme; kiểm tra không cắt ảnh hoặc mất focus ở fade.
- [ ] Chạy `rtk cargo fmt --check`, `rtk cargo test`, `rtk cargo clippy --all-targets -- -D warnings`, build Windows và smoke test bằng file thật.
- [ ] Rà caller/graph trước dọn mã chết; cập nhật README với cách chạy, format đã hỗ trợ và hạn chế thật; graph detect_changes trước commit.
- [ ] Commit thay đổi đã kiểm chứng; ghi rõ kiểm thử nào chưa chạy, không gọi mockup là ứng dụng hoàn chỉnh.
