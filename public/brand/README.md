# LibraTable — Brand assets

Logo, ikon koneksi database, dan ilustrasi empty state. Semuanya SVG (vektor) kecuali PNG/ICO hasil ekspor.
Buka `overview.html` di browser untuk melihat semuanya sekaligus (mengikuti mode terang/gelap sistem).

## Konsep

Nama **Libra** = timbangan. Logo menimbang dua hal yang jadi inti aplikasi: **database** (silinder, kiri) dan
**tabel** (grid, kanan). Timbangannya seimbang: data dan strukturnya setara. Titik emas di pivot dan header
tabel adalah satu-satunya aksen hangat. Konsep ini sama dengan ilustrasi timbangan di `WorkspaceWelcome.tsx`,
jadi logo dan layar welcome terasa satu keluarga.

Warna: biru `#3e63dd` (accent yang sudah dipakai di `tokens.css`), indigo gelap untuk kedalaman, emas `#f2b24c`.
Wordmark: Inter Display. "Libra" SemiBold + "Table" Regular. Teks sudah di-outline jadi path, jadi tidak butuh font.

## Isi folder

```
logo/
  app-icon.svg            ikon aplikasi 1024 (margin macOS + bayangan)
  favicon.svg / .ico      versi full-bleed untuk ukuran kecil (16-256)
  logo-mark.svg           timbangan saja, untuk latar terang
  logo-mark-white.svg     timbangan saja, untuk latar gelap
  logo-mark-mono.svg      satu warna (currentColor)
  logo-horizontal(-dark).svg  ikon + wordmark
  logo-stacked(-dark).svg      ikon di atas wordmark
  banner.svg              banner README 1280x400
  png/                    ekspor PNG (app-icon 16..1024, favicon, logo @2x, banner)
drivers/
  badge/<driver>.svg      21 badge bulat 96x96 (dipakai DriverLogo.tsx)
  glyph/<driver>.svg      glyph saja 48x48, pakai currentColor
empty-states/*.svg        10 ilustrasi 240x170
overview.html / overview-*.png
```

Nama file driver sama persis dengan union `Driver` di `src/types/connection.ts`
(`postgres`, `cloudflare_d1`, `elasticsearch`, ...). Glyph dibuat orisinal dan abstrak (silinder, kunci, chip, dll.),
bukan logo resmi vendor.

## Yang sudah dipasang di aplikasi

- **Ikon aplikasi**: `src-tauri/icons/*` dibuat dengan `bun tauri icon public/brand/logo/png/app-icon-1024.png`;
  `icon.ico` (Windows) memakai `logo/favicon.ico` yang full-bleed. `src-tauri/app-icon.png` = `app-icon-1024.png`,
  `public/app-icon.png` (logo di layar welcome) = `app-icon-256.png`.
- **Badge driver**: `DriverCircle` (`src/components/DriverLogo.tsx`) memuat `/brand/drivers/badge/<driver>.svg`.
- **Empty state**: `EmptyState` dan dialog error menerima `art="<nama file>"`. `vite.brand.ts` menyediakan SVG-nya
  sebagai modul `virtual:brand-art`, dan `BrandArt.tsx` memilih warna terang/gelap sesuai tema **aplikasi**
  (bukan hanya tema sistem). Ilustrasi hanya tampil bila areanya cukup besar; area kecil tetap memakai ikon.
- **Build**: `vite.brand.ts` memangkas `dist/brand` sampai tinggal `drivers/badge/`, jadi file desain lain
  (overview, PNG ekspor, README) tidak ikut masuk ke aplikasi.

Kalau ikon aplikasi diganti lagi, ulangi `bun tauri icon …`, hapus folder `android/` dan `ios/` yang ikut dibuat,
lalu salin ulang `favicon.ico` ke `src-tauri/icons/icon.ico`.

| File | Dipakai untuk (komponen yang sekarang memakai `EmptyState`) |
| --- | --- |
| `no-connections` | `Home.tsx` "No connections yet" |
| `no-results` | `Home.tsx` "No matching connections" |
| `no-tables` | `DiagramView.tsx` "No tables yet" |
| `query-results` | `ResultsPane.tsx` "Results appear here" |
| `no-history` | `HistoryView.tsx` "No history yet" |
| `console` | `ConsolePanel.tsx` "No statements yet" |
| `board` | `MetricsView.tsx` "This board is empty" |
| `no-rows` | `DataGrid.tsx` "No rows." |
| `connection-failed` | dialog "Couldn't connect" (layar awal dan form koneksi) |
| `key-missing` | `ValuePanel.tsx` "This key no longer exists" |

Ilustrasi 240x170 tampil bila area induknya minimal 288x340 px; di bawah itu `EmptyState` memakai ikon lucide.
