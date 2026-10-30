# Puw Player

Trình nghe nhạc offline cho Windows, giao diện theo thiết kế đã duyệt: cover flow, playlist, lời bài hát đồng bộ tùy bật.

- Rust + Slint cho giao diện, Rodio/Symphonia cho âm thanh, SQLite cho thư viện.
- Không cần tài khoản, không thuê bao. Chỉ tra lời online khi người dùng bật LRCLIB.

## Chạy

Yêu cầu: Windows 10/11, Rust 1.92 trở lên (MSVC toolchain).

```bash
cargo run --release
```

Bản debug: `cargo build && target/debug/puw-player.exe`.

Cơ sở dữ liệu nằm tại `%APPDATA%\puw-player\library.db`. Lần chạy đầu thư viện trống; thêm nhạc qua nút **+** ở góc trên (tệp hoặc thư mục).

## Tính năng

- Thêm nhạc từ tệp hoặc thư mục; chỉ lưu đường dẫn, không di chuyển/sao chép/xóa file gốc.
- Thư viện, playlist (tạo/đổi tên/xóa, bỏ bài khỏi playlist) lưu qua SQLite. Rê chuột lên bài ở tab Bài hát → nút **+** để thêm vào playlist có sẵn hoặc tạo playlist mới.
- Hàng đợi khi phát: lặp Tắt → Tất cả → Một bài (icon có số 1); shuffle trộn mọi bài còn lại của hàng đợi, có lịch sử để Previous quay đúng bài; Phát tất cả khi bật shuffle bắt đầu từ bài ngẫu nhiên.
- Morph icon: Play ↔ Pause biến hình liên tục.
- Cover flow từ chính hàng đợi (2 bìa mỗi bên, phối cảnh); Next/Previous trượt cả dải một nấc, tắt khi Windows giảm chuyển động. Bìa chính giữ tỷ lệ, không crop, không phóng to quá 256×256.
- Lời: ưu tiên `.lrc` cùng tên, rồi cache, rồi LRCLIB (mặc định tắt). LRCLIB lỗi 502/503/504 được tự thử lại một lần. Click dòng có timestamp để tua; nút "Theo bài hát" khôi phục bám dòng; chỉnh lệch ±0,5 giây lưu riêng cho từng bài.
- Theme Hệ thống/Sáng/Tối; DWM bo góc 12 px khi không maximize; tôn trọng cài đặt giảm chuyển động của Windows.
- Đăng ký Windows Media Controls (SMTC): tên bài, nghệ sĩ, album, ảnh bìa, tiến trình hiện ở bảng media của Windows và các app đọc "now playing" như puw-notch; nhận lại Play/Pause/Next/Previous/Stop/tua và phím media trên bàn phím.
- Phím: `Space` phát/tạm dừng, `←`/`→` tua ±10 giây, `↑`/`↓` âm lượng, `Esc` đóng lớp đang mở.

## Định dạng đã kiểm chứng

- FLAC 16-bit: đọc metadata, bìa nhúng, phát và tua (test tự động + file thật).
- WAV PCM: metadata, duration, phát và tua (test tự động).
- M4A/AAC-LC: đọc metadata + phát file thật.
- MP3: nhập và đọc metadata từ file thật; **chưa** kiểm thử phát end-to-end trong repo này.

Chưa hỗ trợ/cam kết: WMA, APE, DSD, HE-AAC, Opus, bit-perfect/exclusive output.

## Hạn chế đã biết

- Cửa sổ không viền có kéo, resize, minimize/maximize/close; **Snap Layouts của Windows chưa được kiểm chứng**.
- Chưa có kéo-thả tệp vào cửa sổ; thêm nhạc qua menu **+**.
- Lời online cần mạng; mặc định tắt. Một số bài không có lời hoặc là bản không lời sẽ hiển thị thông báo.
- Bìa chỉ lấy từ tag nhúng trong file; không đọc `cover.jpg` cạnh file.
- DPI đã kiểm tra ở mức hiển thị hiện tại (100%); chưa chụp đối chiếu 125%/150% trong lần này.

## Kiểm thử

```bash
cargo fmt --check
cargo test --no-default-features   # 24 test core: playback, queue, artwork, lyrics, library, settings
cargo clippy --all-targets -- -D warnings
cargo build
```

Test chạy không cần thiết bị âm thanh (engine có output giả lập). Giao diện được kiểm tra thủ công bằng thao tác thật trên Windows: phát/tạm dừng, tua, đổi bài, playlist, lyrics, theme, maximize/restore, resize về 960 px.

## Cấu trúc

- `src/playback.rs` — engine phát (generation chống response cũ), Rodio output, worker thread.
- `src/queue.rs` — hàng đợi, shuffle/lịch sử, repeat.
- `src/library.rs` — SQLite: tracks, playlists, settings, cache lời, import.
- `src/lyrics.rs` — parser LRC, tìm dòng hiện tại, nguồn local/cache/LRCLIB.
- `src/settings.rs` — theme và bật/tắt lời online.
- `ui/player.slint` — toàn bộ giao diện.
- `src/media_keys.rs` — Windows Media Controls (SMTC) qua `souvlaki`; ảnh bìa xuất PNG tạm vào `%TEMP%\puw-player`.
- `src/main.rs` — nối UI với core, DWM, theme hệ thống, cover loader.
