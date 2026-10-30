# Puw Player: thiết kế đã duyệt

Ngày duyệt: 2026-10-30. Người dùng chốt ảnh Cover Flow cuối cùng và yêu cầu bo nhẹ góc.

## Hình mẫu

Tham chiếu: `../../design/approved-player.png`. Ảnh là chuẩn bố cục, không phải bằng chứng ứng dụng đã hoạt động. Kích thước dưới đây dùng đơn vị giao diện logic, không phải pixel vật lý của ảnh sinh.

## Cửa sổ và hình thức

- Windows desktop, Rust; hướng triển khai Slint và Rodio/Symphonia.
- Cửa sổ mặc định 1200×680; kích thước nhỏ nhất dự kiến 960×600, phải kiểm chứng khi triển khai.
- Không dùng titlebar Windows tiêu chuẩn; có vùng kéo riêng, resize, minimize, maximize, close và kiểm thử Snap.
- Bo góc cửa sổ 12 px, bìa 6 px là giá trị triển khai đề xuất cho yêu cầu bo nhẹ. Khi maximize bỏ bo góc ngoài để khít màn hình.
- Nền graphite phẳng, chữ sáng, một màu nhấn xanh dịu; không kính mờ hoặc bóng nổi. Nút player tròn.
- Line chia vùng mảnh, fade trong suốt hai đầu. Danh sách có fade tại mép còn nội dung cuộn; mép đã tới đầu/cuối không che mất dòng cuối.
- Hỗ trợ theme hệ thống/sáng/tối; ảnh duyệt thể hiện theme tối.

## Cover Flow và player bên trái

- Bìa chính tối đa 256×256; vùng trình bày khoảng 520×300. Giữ tỷ lệ ảnh, contain vào khung, không crop hoặc kéo méo; ảnh nhỏ không bắt buộc phóng lên.
- Bìa trước/sau lấy từ hàng đợi, nghiêng phối cảnh và giảm opacity theo khoảng cách, theo hình mẫu người dùng cung cấp. Không giả bìa lân cận khi hàng đợi thiếu bài.
- Tên bài, nghệ sĩ và thông số nguồn: codec, lossless/lossy, sample rate, bit depth khi có; thông số thiết bị xuất tách khỏi thông số file.
- Timeline hiển thị vị trí và tổng thời lượng, hỗ trợ click/kéo/phím; tua ±10 giây, Previous/Next, Play/Pause, Stop, Shuffle, Repeat và âm lượng.
- Stop đưa vị trí về đầu bài; Repeat chuyển Off → All → One. All lặp hàng đợi hiện tại, không tự kéo toàn bộ thư viện vào.
- Shuffle áp dụng trong hàng đợi hiện tại, có lịch sử để Previous quay đúng bài; tránh lặp trước khi hết lượt.
- Morphicons cho chuyển trạng thái Play/Pause và Repeat; cùng hộp icon, không dịch bố cục. Tôn trọng giảm chuyển động.

## Khu vực bên phải

- Hai tab Bài hát và Playlist. Playlist có danh sách riêng và bài trong playlist đang chọn; có tạo playlist, thêm/xóa bài và đổi tên. Xóa khỏi playlist không xóa file âm thanh.
- Phát playlist tạo hàng đợi từ playlist đó; xem playlist khác không tự đổi bài đang phát.
- Thêm nhạc từ file, thư mục hoặc kéo thả; lưu tham chiếu đường dẫn, không di chuyển/sao chép file gốc.
- Icon Lyrics bật/tắt vùng lời ở bên phải, tắt trả lại tab và vị trí danh sách trước đó. Cài đặt là icon mở khu vực riêng; phát nhạc không bị ngắt.
- Trạng thái trống, đang đọc dữ liệu, file mất/hỏng phải rõ và không làm treo player.

## Lời bài hát

- Ưu tiên file .lrc cùng tên, cache local, rồi LRCLIB nếu tải lời online được bật.
- Tra metadata tên bài/nghệ sĩ/album/thời lượng, fallback tìm kiếm để chọn đúng bản, không chọn mù kết quả đầu.
- Đồng bộ timestamp theo vị trí phát thực tế, tính lại sau seek/pause/repeat; lưu offset riêng cho bài.
- Lời thường hiển thị tĩnh; nhạc instrumental hoặc không tìm thấy có thông báo phù hợp. Tải lời thất bại không chặn phát nhạc.
- Cuộn thủ công tạm ngừng bám dòng; nút Theo bài hát khôi phục bám dòng. Click dòng có thời gian để seek.
- Cache lời cùng nguồn/ID; bỏ response cũ khi đổi bài; User-Agent nhận diện app; tuân thủ Retry-After khi 429, không gửi đường dẫn local cho dịch vụ.

## Phạm vi và kiểm chứng

- Nhạc offline không cần tài khoản hoặc thuê bao. Chỉ tra lời cần mạng; lời đã cache dùng offline.
- Định dạng bản đầu dự kiến MP3, FLAC, WAV PCM, Ogg Vorbis, M4A/MP4 chứa AAC-LC/ALAC và AAC-LC; chỉ công bố hỗ trợ sau kiểm thử codec/container thực.
- Chưa cam kết WMA, APE, DSD, HE-AAC, Opus hoặc xuất bit-perfect/exclusive.
- Kiểm thử DPI 100/125/150%, thao tác bàn phím, focus, tương phản, ảnh ngang/dọc, mất thiết bị âm thanh, đổi bài liên tục, playlist rỗng, file hỏng và chạy không mạng.
- Không tạo requirements.txt cho dependency Rust; dependency Rust thuộc Cargo.toml. Chưa cài dependency trong lần chốt thiết kế này.
