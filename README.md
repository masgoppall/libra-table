<p align="center">
  <img src="public/brand/logo/png/banner.png" alt="LibraTable, desktop database client" width="100%">
</p>

<p align="center">
  <strong>Satu aplikasi desktop untuk semua database Anda.</strong><br>
  Buka, telusuri, ubah, dan kelola data dengan cepat, tanpa takut salah ubah di server produksi.
</p>

<p align="center">
  <img alt="Versi 0.3.0" src="https://img.shields.io/badge/versi-0.3.0-3e63dd">
  <img alt="macOS 13 atau lebih baru" src="https://img.shields.io/badge/macOS-13%2B-1e3190">
  <img alt="Dibuat dengan Tauri 2 dan React" src="https://img.shields.io/badge/Tauri%202-React-f2b24c">
</p>

---

## Kenapa LibraTable?

- **Satu tempat untuk banyak database.** PostgreSQL, MySQL, MariaDB, SQLite, Snowflake, BigQuery, DynamoDB, dan Redis dalam satu jendela, masing-masing di tab sendiri.
- **Terasa seperti aplikasi Mac.** Ringan, cepat dibuka, mengikuti mode terang/gelap, dan hampir semua aksi punya pintasan keyboard.
- **Aman untuk produksi.** Setiap perubahan ditampilkan dulu sebagai SQL sebelum dijalankan. Koneksi bertanda `production` meminta konfirmasi, dan koneksi read-only benar-benar ditolak oleh server.
- **Password tidak pernah disimpan di file.** Semua rahasia ada di Keychain macOS.
- **Pindah dari TablePlus dalam satu langkah.** Koneksi, grup, password, dan kunci SSH ikut terbawa.

## Tampilan

<p align="center">
  <img src="docs/screenshots/welcome.png" alt="Layar awal: daftar koneksi dengan tag lingkungan" width="49%">
</p>

| Data tabel | Struktur tabel |
| --- | --- |
| ![Grid data dengan baris belang, tipe kolom, dan NULL](docs/screenshots/data.png) | ![Tab Structure dengan menu tipe data, nullable, default, dan extra](docs/screenshots/structure.png) |

## Database yang didukung

| | Database | Status |
| --- | --- | --- |
| <img src="public/brand/drivers/badge/postgres.svg" width="22" alt=""> | PostgreSQL | Siap dipakai |
| <img src="public/brand/drivers/badge/mysql.svg" width="22" alt=""> | MySQL | Siap dipakai |
| <img src="public/brand/drivers/badge/mariadb.svg" width="22" alt=""> | MariaDB | Siap dipakai |
| <img src="public/brand/drivers/badge/sqlite.svg" width="22" alt=""> | SQLite | Siap dipakai |
| <img src="public/brand/drivers/badge/snowflake.svg" width="22" alt=""> | Snowflake | Siap dipakai (diuji dengan tiruan layanan) |
| <img src="public/brand/drivers/badge/bigquery.svg" width="22" alt=""> | BigQuery | Siap dipakai (diuji dengan emulator) |
| <img src="public/brand/drivers/badge/dynamodb.svg" width="22" alt=""> | DynamoDB | Siap dipakai (diuji dengan emulator) |
| <img src="public/brand/drivers/badge/redis.svg" width="22" alt=""> | Redis | Siap dipakai |
| <img src="public/brand/drivers/badge/cockroach.svg" width="22" alt=""> | CockroachDB, Redshift, Greenplum | Memakai mesin PostgreSQL, belum ada penyesuaian khusus |
| <img src="public/brand/drivers/badge/mssql.svg" width="22" alt=""> | SQL Server, Oracle, ClickHouse, DuckDB, Cassandra, MongoDB, Elasticsearch, LibSQL, Cloudflare D1, Vertica | Segera hadir (tampil abu-abu di pemilih driver) |

## Fitur

**Koneksi**
- Langsung atau lewat **SSH tunnel** (password atau kunci, membaca `~/.ssh/config`, memeriksa host key) dan **SSL/TLS**.
- Grup bertingkat dengan ikon sendiri, tag lingkungan berwarna (`local`, `development`, `testing`, `staging`, `production`), dan duplikat koneksi.
- Impor dari URL (`postgres://…`, `mysql://…`), dari file ekspor LibraTable terenkripsi, atau dari TablePlus.
- Tidak ada koneksi otomatis: tab terakhir dipulihkan, tetapi baru tersambung saat Anda klik.

**Data**
- Grid cepat untuk ribuan baris: urutkan, sembunyikan atau pindahkan kolom, ubah lebar, dan pindah halaman.
- **Filter** per kolom atau di semua kolom, termasuk *contains*, *between*, *in list*, dan Raw SQL. Tekan ↵ untuk menerapkan, **Clear** untuk menampilkan semua baris lagi.
- Edit sel langsung di grid, tambah atau hapus baris, lalu commit sekaligus. ⌘Z membatalkan, dan dropdown enum serta pemilih tanggal tersedia.
- **Quick Look** (Spasi) untuk JSON, gambar, dan data biner. Klik foreign key untuk membuka baris yang dirujuk.

**Struktur tabel**
- Tab **Data | Structure | Index** di tab yang sama: ubah nama, tipe, nullable, default, extra, charset, collation, dan komentar kolom lewat menu atau ketik langsung.
- Tarik kolom untuk mengubah urutan (MySQL), buat foreign key dari selnya, tambah index, lihat trigger, dan lihat DDL.
- Semua perubahan ditampilkan sebagai SQL dulu, lalu dijalankan dengan ⌘S.

**Query**
- Editor SQL dengan pelengkapan otomatis, format (⌥⌘F), Explain (⌥⌘E), dan variabel `:nama`.
- Jalankan statement di kursor (⌘↵) atau semuanya (⇧⌘↵), batalkan query yang sedang berjalan (⌘.), simpan query favorit, dan cari di **riwayat**.

**Impor, ekspor, dan alat server**
- Impor CSV/JSON ke tabel yang ada atau tabel baru, dan jalankan file SQL besar.
- Ekspor ke CSV, JSON, atau SQL, termasuk beberapa tabel sekaligus.
- Backup dan restore (`pg_dump`/`mysqldump`), daftar proses (cancel/kill), serta pengguna dan hak akses.

**Visual**
- **Diagram ER** yang bisa digeser dan diekspor ke SVG/PNG.
- **Papan metrik** berisi angka, tabel, grafik batang, garis, dan pie dengan refresh otomatis.

## Mulai

### Kebutuhan

- macOS 13 atau lebih baru (Apple Silicon sudah diuji)
- [Rust](https://rustup.rs) 1.82 atau lebih baru, beserta Xcode Command Line Tools
- [Node.js](https://nodejs.org) 22 dan [Bun](https://bun.sh) 1.3 atau lebih baru

### Jalankan

```bash
bun install
bun tauri dev                    # aplikasi dengan hot reload
```

### Buat aplikasinya

```bash
bun tauri build --bundles app    # LibraTable.app di src-tauri/target/release/bundle/macos
bun tauri build                  # .app + .dmg (jalankan dari sesi desktop; langkah DMG memakai Finder)
```

> Ingin mencoba tanpa menyentuh koneksi yang sudah tersimpan? Jalankan dengan
> `LIBRATABLE_DATA_DIR=/folder/kosong`.

## Pindah dari TablePlus

1. Di TablePlus, ekspor koneksi Anda menjadi file `.tableplusconnection` dan ingat password ekspornya.
2. Di LibraTable, klik **+** di layar awal, lalu **Import Connection → From TablePlus…**. Bisa juga klik kanan area kosong daftar koneksi, lalu pilih **Import from TablePlus…**.
3. Pilih file tersebut dan masukkan password ekspornya.

Koneksi, grup bertingkat, password, dan kunci SSH ikut terbawa. Koneksi yang sudah ada di LibraTable dilewati, jadi impor aman diulang.

## Pintasan keyboard

| Tombol | Aksi |
| --- | --- |
| ⌘N | Koneksi baru |
| ⌘P | Buka tabel, view, atau query |
| ⌘K | Pindah database |
| ⌘T / ⌘W | Tab query baru / tutup tab |
| ⌘F | Filter baris |
| ⌘S | Simpan query, commit perubahan, atau terapkan struktur |
| ⌘R | Muat ulang |
| ⇧⌘S | Tampilkan struktur tabel |
| ⌘↵ / ⇧⌘↵ | Jalankan statement di kursor / semua statement |
| ⇧⌘C | Log console (setiap statement yang dikirim ke server) |
| ⇧⌘L | Kunci LibraTable dengan passcode |
| ⌘/ | Lihat semua pintasan (bisa diubah) |

## Keamanan dan privasi

- **Rahasia hanya di Keychain macOS.** Password, kunci SSH, dan passphrase disimpan di Keychain (layanan `com.libratable.app`). File konfigurasi tidak pernah berisi rahasia.
- **Nilai selalu dikirim sebagai parameter.** Filter dan edit tidak pernah menempelkan nilai ke dalam SQL.
- **Safe mode dijaga oleh backend.** Perubahan pada koneksi `production` (atau sesuai pengaturan per koneksi) harus dikonfirmasi. Mode "safe" bahkan meminta Anda mengetik nama koneksi.
- **Read-only sungguhan.** Koneksi read-only ditolak oleh aplikasi dan oleh sesi server (`READ ONLY`).
- **Passcode** untuk mengunci aplikasi, dengan kunci otomatis setelah tidak dipakai.

Data aplikasi disimpan di `~/Library/Application Support/com.libratable.app/`.

## Performa

Diukur dengan build release di Mac Apple Silicon:

| Uji | Hasil |
| --- | --- |
| Mencari satu kata di 100.000 entri riwayat | 11 ms |
| Ekspor 5.000.000 baris ke CSV | memori naik hanya +2 MB (file 264 MB dalam 10,2 detik) |
| Bundel JS utama | 163 kB (gzip). Editor SQL, diagram ER, metrik, dan ilustrasi dimuat terpisah |

## Keterbatasan saat ini

- Baru diuji di macOS (arm64). Build Windows dan Linux belum diuji.
- Belum ada auto-update dan notarization. Build lokal ditandatangani ad-hoc, jadi setelah build ulang macOS meminta izin Keychain sekali per koneksi.
- Urutan kolom hanya bisa diubah di MySQL; PostgreSQL dan SQLite tidak mendukung pemindahan kolom.
- Nilai biner belum bisa diedit (bisa dilihat di Quick Look dan diekspor utuh).
- `verify-full` PostgreSQL lewat SSH tunnel gagal memverifikasi hostname.
- Index pada kolom TEXT/BLOB MySQL belum bisa diberi panjang prefix dari editor index.

---

## Untuk pengembang

### Teknologi

**Tauri 2 (Rust, Tokio, sqlx)** untuk inti aplikasi, serta **React 18, TypeScript, Zustand, dan Vite** untuk antarmuka.

```
React UI (webview)  ──invoke / events──▶  Rust core
```

### Tes dan lint

```bash
bun run check        # tsc + eslint + prettier --check + vitest
bun run test         # vitest saja (jangan `bun test`, itu runner milik Bun)
cd src-tauri && cargo test
cargo fmt --check && cargo clippy --all-targets -- -D warnings
```

Tes end-to-end PostgreSQL dan MySQL berjalan bila URL admin diberikan. Tanpa URL itu, hanya SQLite yang diuji. Setiap suite membuat database `libratable_*` sendiri dan menghapusnya lagi.

```bash
LIBRATABLE_TEST_PG_URL=postgres://postgres@127.0.0.1:5432/postgres \
LIBRATABLE_TEST_MYSQL_URL=mysql://root@127.0.0.1:3306/mysql \
cargo test
```

### Struktur folder

```
src/                 antarmuka React
  app/               shell, toolbar, menu, pintasan global
  features/          satu folder per layar (connections, data, structure, query, …)
  components/        komponen bersama (grid, dialog, menu, ilustrasi brand)
  store/ services/ api/ domain/ types/ lib/ styles/
src-tauri/src/       inti Rust
  commands/          perintah IPC per domain
  connections/       simpan, sambung, SSH, impor/ekspor koneksi (termasuk TablePlus)
  db/                pool, filter, DDL, metadata per driver
  engines/           Snowflake, BigQuery, DynamoDB, Redis, …
  storage/           konfigurasi JSON, Keychain, riwayat (SQLite + FTS5)
public/brand/        logo, badge driver, dan ilustrasi empty state
```

Aturan kode: baris paling panjang 100 karakter (Prettier/ESLint dan `rustfmt`), TypeScript strict tanpa `any`, dan arah dependensi `features → services/store → api → types`.

Aset brand dipakai lewat `vite.brand.ts`: ilustrasi empty state dibaca sebagai modul, sehingga warnanya mengikuti tema aplikasi. Saat build, isi `brand/` di folder `dist` dipangkas sampai tinggal badge driver, jadi file desain lain tidak ikut ke dalam aplikasi.
