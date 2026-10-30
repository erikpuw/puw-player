<p align="center"><img src="assets/puw_1024.png" width="96" alt="Puw Player"></p>

<h1 align="center">Puw Player</h1>

<p align="center">Trình nghe nhạc offline cho Windows · <a href="README.en.md">English</a></p>

Puw Player phát **nhạc có sẵn trên máy bạn**. Không stream, không tài khoản, không thuê bao, không gắn với Spotify, Apple Music hay nền tảng nhạc nào. Kết nối mạng duy nhất là tra **lời bài hát** cho các bài bạn đã có (LRCLIB, mặc định tắt).

## Tính năng

| Nhóm | Có gì |
|---|---|
| Thư viện | Thêm tệp hoặc cả thư mục. Chỉ lưu đường dẫn, không sao chép, di chuyển hay xóa file gốc. |
| Phát nhạc | Phát/tạm dừng, dừng, bài trước/sau, tua ±10 giây, kéo thanh thời gian, âm lượng. |
| Hàng đợi | Shuffle không lặp bài, Previous quay đúng bài vừa nghe. Lặp: tắt / tất cả / một bài. |
| Playlist | Tạo, đổi tên, xóa. Thêm bài từ tab Bài hát, bỏ bài khỏi playlist. Xóa playlist không xóa file nhạc. |
| Chất lượng | Sau thông số nguồn có huy hiệu: **Hi-Res Lossless** (lossless 24-bit hoặc trên 48 kHz), **Lossless** (FLAC, ALAC, WAV…), hoặc bitrate cho file nén (vd. 320 kbps). Bản phát hành ghi "Dolby Atmos"/"Binaural" trong tên có thêm nhãn kèm icon tai nghe. |
| Cover flow | Bìa đang phát ở giữa, 2 bìa mỗi bên theo hàng đợi, trượt khi chuyển bài. |
| Lời bài hát | Ưu tiên file `.lrc` cạnh bài hát, rồi lời đã lưu, rồi LRCLIB nếu bạn bật. Lời chạy theo nhạc, bấm một dòng để tua, chỉnh lệch ±0,5 giây cho từng bài. Lời đã tải dùng được khi offline. |
| Windows | Hiện trong bảng media của Windows và nhận phím media trên bàn phím. Các app đọc "now playing" như PUW Notch cũng thấy được. |
| Giao diện | Theme hệ thống / sáng / tối, tiếng Việt / English, tôn trọng cài đặt giảm chuyển động của Windows. |
| Phím tắt | `Space` phát/tạm dừng · `←` `→` tua 10 giây · `↑` `↓` âm lượng · `Esc` đóng lớp đang mở. |

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

## Phát triển

```bash
cargo test --no-default-features     # test lõi, không cần thiết bị âm thanh
cargo clippy --all-targets -- -D warnings
```

Rust + [Slint](https://slint.dev) cho giao diện, Rodio/Symphonia cho âm thanh, SQLite cho dữ liệu. Icon vẽ bằng `assets/make_icon.py`.

## Hạn chế

- Chưa kéo-thả file vào cửa sổ; thêm nhạc qua nút **+**.
- Ảnh bìa chỉ lấy từ ảnh nhúng trong file, chưa đọc `cover.jpg` trong thư mục.
- Snap Layouts của Windows và màn hình DPI 125%/150% chưa được kiểm tra kỹ.
