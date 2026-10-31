<p align="center"><img src="assets/puw_1024.png" width="96" alt="Puw Player"></p>

<h1 align="center">Puw Player</h1>

<p align="center">Trình nghe nhạc offline cho Windows · <a href="README.en.md">English</a></p>

Puw Player phát **nhạc có sẵn trên máy bạn**. Không stream, không tài khoản, không thuê bao, không gắn với Spotify, Apple Music hay nền tảng nhạc nào. Kết nối mạng duy nhất là tra **lời bài hát** cho các bài bạn đã có (LRCLIB, mặc định tắt).

## Tính năng

| Nhóm | Có gì |
|---|---|
| Thư viện | Thêm tệp hoặc cả thư mục. Chỉ lưu đường dẫn, không sao chép, di chuyển hay xóa file gốc. Xóa bài khỏi thư viện (nút thùng rác trên dòng) hoặc dọn mọi bài không còn file (Cài đặt → Dọn bài lỗi); file nhạc không bị xóa. |
| Phát nhạc | Phát/tạm dừng, dừng, bài trước/sau, tua ±10 giây, kéo thanh thời gian, âm lượng. |
| Hàng đợi | Shuffle không lặp bài, Previous quay đúng bài vừa nghe. Lặp: tắt / tất cả / một bài. |
| Tìm bài | Ô tìm ở tab Bài hát, lọc nhanh theo tên bài hoặc nghệ sĩ. |
| Playlist | Tạo, đổi tên, xóa. Thêm bài từ tab Bài hát, bỏ bài khỏi playlist. Xóa playlist không xóa file nhạc. |
| Chất lượng | Sau thông số nguồn có huy hiệu: **Hi-Res Lossless** (lossless 24-bit hoặc trên 48 kHz), **Lossless** (FLAC, ALAC, WAV…), hoặc bitrate cho file nén (vd. 320 kbps). Bản phát hành ghi "Dolby Atmos"/"Binaural" trong tên có thêm nhãn kèm icon tai nghe. |
| Cover flow | Bìa đang phát ở giữa, 2 bìa mỗi bên theo hàng đợi, trượt khi chuyển bài. |
| Lời bài hát | Ưu tiên file `.lrc` cạnh bài hát, rồi lời đã lưu, rồi LRCLIB nếu bạn bật. Lời chạy theo nhạc, bấm một dòng để tua, chỉnh lệch ±0,5 giây cho từng bài. Lời đã tải dùng được khi offline. |
| Windows | Hiện trong bảng media của Windows và nhận phím media trên bàn phím. Các app đọc "now playing" như PUW Notch cũng thấy được. |
| Giao diện | Theme hệ thống / sáng / tối, tiếng Việt / English, tôn trọng cài đặt giảm chuyển động của Windows. |
| Phím tắt | `Space` phát/tạm dừng · `←` `→` tua 10 giây · `↑` `↓` âm lượng · `Esc` đóng lớp đang mở. |

## Ảnh chụp

| | |
|---|---|
| ![Đang phát](assets/screenshots/player-dark.png)<br>**Đang phát**: bìa ở giữa, hai bìa kề mỗi bên lấy theo hàng đợi, trượt khi chuyển bài | ![Chất lượng](assets/screenshots/player-quality.png)<br>**Chất lượng**: FLAC 24-bit / 96 kHz nên có nhãn **Hi-Res Lossless**; bản ghi "Dolby Atmos" trong tên có thêm nhãn tai nghe |
| ![Lời bài hát](assets/screenshots/player-lyrics.png)<br>**Lời bài hát**: dòng đang hát sáng lên, bấm một dòng để tua, chỉnh lệch ±0,5 giây | ![Playlist](assets/screenshots/player-playlist.png)<br>**Playlist**: tạo, đổi tên, thêm và bỏ bài; không đụng tới file nhạc |
| ![Cài đặt](assets/screenshots/player-settings.png)<br>**Cài đặt**: theme, ngôn ngữ, tải lời từ LRCLIB, dọn bài lỗi | ![Theme sáng](assets/screenshots/player-light.png)<br>**Theme sáng** — giao diện theo hệ thống / sáng / tối |

Ảnh chụp dùng thư viện mẫu tự dựng (tên bài, nghệ sĩ, album, ảnh bìa và lời đều là dữ liệu demo do dự án sinh ra) để không dính bản quyền của bài hát thật.

## Riêng tư

| Gửi ra ngoài | Khi nào |
|---|---|
| Tên bài, nghệ sĩ, album, thời lượng | Chỉ khi bật "Tải lời từ LRCLIB" và bài chưa có lời. |
| Đường dẫn file, nội dung nhạc | Không bao giờ. |

## Định dạng

| Định dạng | Trạng thái |
|---|---|
| FLAC, WAV PCM, M4A (AAC-LC) | Đã thử phát thật. |
| MP3 | Đọc được thông tin bài; chưa thử kỹ việc phát. |
| WMA, APE, DSD, Opus, HE-AAC | Chưa hỗ trợ. |

## Cài và chạy

Cần Windows 10/11 và [Rust](https://rustup.rs) 1.92 trở lên (MSVC toolchain).

```bash
cargo build --release
```

File chạy: `target/release/puw-player.exe`. Lần đầu thư viện trống; thêm nhạc bằng nút **+** ở góc trên. Dữ liệu (thư viện, playlist, cài đặt, lời đã tải) nằm trong `%APPDATA%\puw-player\library.db`.
Exe mang icon và thông tin phiên bản (Properties → Details) lấy từ `version` trong `Cargo.toml`. Exe chưa ký số nên khi gửi qua mạng, SmartScreen hoặc antivirus vẫn có thể cảnh báo; khách bấm Properties → **Unblock** trên file zip trước khi giải nén.

## Phát triển

```bash
cargo test --no-default-features     # test lõi, không cần thiết bị âm thanh
cargo clippy --all-targets -- -D warnings
```

Rust + [Slint](https://slint.dev) cho giao diện, Rodio/Symphonia cho âm thanh, SQLite cho dữ liệu. Icon vẽ bằng `assets/make_icon.py`.

## Giấy phép

[MIT](LICENSE)

## Hạn chế

- Chưa kéo-thả file vào cửa sổ; thêm nhạc qua nút **+**.
- Ảnh bìa chỉ lấy từ ảnh nhúng trong file, chưa đọc `cover.jpg` trong thư mục.
- Snap Layouts của Windows và màn hình DPI 125%/150% chưa được kiểm tra kỹ.
