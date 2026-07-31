# ctx-agent

> Kod tabanını AI agent'ların anlayacağı hale getiren küçük bir Rust CLI'si.

![Rust](https://img.shields.io/badge/rust-stable-orange)
![SQLite](https://img.shields.io/badge/sqlite-FTS5-blue)
![MCP](https://img.shields.io/badge/MCP-compatible-green)
![License](https://img.shields.io/badge/license-MIT-lightgrey)

Kısaca: agent'a "bu dosyayı değiştirirsem ne bozulur?" sorusunu kodu çalıştırmadan
cevaplayabileceği bir hafıza veriyor.

## Neden yazdım

AI agent'larla (Codex, Claude vs.) büyük repolarda çalışırken en çok vakit
kaybettiren şey agent'ın dosya dosya gezip "acaba bu nereden çağrılıyor" diye
aramasıydı. ctags tarzı araçlar sembol listesi veriyor ama agent'a dönük değiller;
Sourcegraph gibi çözümler sunucu istiyor. İstediğim şey:

- kurcaladığım projeyi bir kere tarasın, SQLite'a yazsın,
- agent MCP ile bağlanıp sembol/dependency/karar geçmişini sorgulasın,
- internet olmadan, API key olmadan çalışsın.

ctx-agent bunu yapıyor. LLM yok, bulut yok, tek binary + SQLite.

## Ne yapar, ne yapmaz

Yapar:

- Projeyi tarayıp dosya/sembol/import çıkarır (tree-sitter ile)
- Dependency graph ve "blast radius" hesaplar
- Git geçmişinden conventional commit'lere bakıp karar kaydı çıkarır
- FTS5 ile sembol araması, kırılgan/büyük/ölü dosya uyarıları
- MCP server üzerinden agent'a servis eder

Yapmaz:

- Kod yazmaz, cevap üretmez (LLM değil)
- Derleme/lint doğruluğu garanti etmez (statik yapısal analiz)
- Runtime davranışı izlemez
- LSP değildir, go-to-definition işi görmez

## Özellikler

| Özellik | Açıklama |
|---------|----------|
| Codebase map | Dizin ağacı, dosya/satır sayıları, dosya başına semboller |
| Sembol çıkarma | Fonksiyon, class, struct, interface, enum, constant — imzalarıyla |
| Dependency graph | Import/export analizi, blast radius |
| Karar takibi | Conventional commit'lerden otomatik karar çıkarma |
| Full-text arama | FTS5, kısmi eşleşme |
| Sağlık uyarıları | Kırılgan dosyalar, ölü kod, büyük dosyalar |
| Bilgi notları | `ctx learn` ile mimari not/gotcha kaydetme |
| File watcher | Değişiklikte canlı yeniden analiz |
| MCP server | Agent'lar Model Context Protocol ile bağlanır |
| JSON çıktı | `--json` ile makine okunabilir çıktı |

## Dil desteği

| Dil | Semboller | Importlar | Durum |
|-----|-----------|-----------|-------|
| Rust | fn, struct, enum, impl, mod | `use` | Tam |
| TypeScript/JavaScript | function, class, interface, type, const | `import`/`export` | Tam |
| Python | def, class, decorator, modül sabitleri (ALL_CAPS) | `import`/`from` | Tam |
| Go | func, struct, interface, type | `import` | Tam |
| C/C++, Java, C#, PHP, Ruby, Shell | Temel semboller | Temel | Kısmi |
| Swift, Kotlin | Dosya takibi + satır sayısı | — | Planlanıyor |

> Sembol çıkarılmayan dillerde bile dosya takibi, satır sayısı ve git geçmişi
> analizi çalışıyor.

## Kurulum

```bash
git clone https://github.com/Ahmetshbzz/ctx-agent.git && cd ctx-agent
cargo build --release
# binary: target/release/ctx
```

İstersen PATH'e at:

```bash
ln -sf $(pwd)/target/release/ctx ~/.local/bin/ctx
```

## Kullanım

```bash
cd projenin-kök-dizini
ctx init
```

İlk tarama birkaç saniye sürer, proje başına bir SQLite açılır:
`~/.ctx-agent/projects/<project-hash>/ctx.db`

Sonrası:

```bash
ctx status                          # proje özeti
ctx map                             # dizin ağacı + sembol sayıları
ctx query "parse"                   # sembol arama
ctx grep "TODO"                     # ham metin arama (rg benzeri, gömülü)
ctx blast-radius src/db/mod.rs      # etki analizi
ctx decisions                       # git'ten çıkarılan kararlar
ctx learn "Auth JWT RS256 kullanıyor"   # bilgi notu ekle
ctx warnings                        # kırılgan/büyük/ölü dosyalar
ctx watch                           # değişiklikte canlı analiz
ctx status --json                   # agent için JSON
```

Sonraki taramalar incremental: sadece değişen dosyalar yeniden analiz edilir
(`ctx scan`).

## MCP server (agent entegrasyonu)

`mcp-server/` altında TypeScript bir MCP server var:

```bash
cd mcp-server
npm install
npm run build
```

MCP config'e ekle:

```json
{
  "mcpServers": {
    "ctx": {
      "command": "node",
      "args": ["/path/to/ctx-agent/mcp-server/dist/index.js"]
    }
  }
}
```

Araçlar: `ctx_init`, `ctx_status`, `ctx_map`, `ctx_scan`, `ctx_query`, `ctx_grep`,
`ctx_blast_radius`, `ctx_decisions`, `ctx_learn`, `ctx_warnings`, `ctx_overview`,
`ctx_guard`.

Bilinen davranışlar:

- Proje initialize edilmemişse ilk MCP çağrısı otomatik `init` çalıştırır.
- `ctx_status`, hiç bilgi notu yoksa ilk seferde bir overview notu oluşturur.
- Agent komutları arka planda watch başlatır (kapatmak için
  `CTX_AGENT_DISABLE_AUTO_WATCH=1`).
- `ctx_query` sembol araması boş dönerse otomatik olarak metin aramasına düşer.
- Paranoid mod varsayılan açık (`CTX_PARANOID=1`): auth/session/token/crypto
  değişikliklerinde `ctx_guard` BLOCK/PASS raporlar.
- Her MCP çağrısı `~/.ctx-agent/activity/<project-hash>.jsonl` günlüğüne yazılır.

## Karar takibi

Conventional commit kullanıyorsan `feat:`, `fix:`, `refactor:` ve
`BREAKING CHANGE:` içeren commit'ler otomatik karar olarak kaydedilir:

```
$ ctx decisions

  Decisions 3

  2026-02-10 [commit] feat(auth): switch to JWT RS256 (a3b8d1)
  2026-02-10 [commit] fix: FTS5 contentless table — use regular FTS5 (37fea0b)
  2026-02-10 [commit] feat: add TypeScript MCP server (55247d9)
```

Commit mesajlarını düzgün yazarsan bedavaya mimari karar günlüğün olur.

## Sağlık uyarıları

| Uyarı | Kural | Anlamı |
|-------|-------|--------|
| Fragile file | `churn_score > 5.0` ve `dependents > 3` | Çok değişen + çok kullanılan dosya |
| Large file | `line_count > 500` | Bölme adayı |
| Dead code | commit yok ve kimse import etmiyor | Muhtemelen silinebilir |

```
$ ctx warnings

  Warnings 2

  Fragile files (high churn + many dependents):
    · src/db/mod.rs — 12 changes, 8 dependents (churn: 7.2)

  Large files (>500 lines):
    · src/analyzer/parser.rs — 618 lines (rust)
```

## CLI referansı

```
Usage: ctx [OPTIONS] <COMMAND>

Commands:
  init          Initialize ctx-agent in the current project
  scan          Scan/re-scan the project (incremental)
  map           Display codebase map with structure and stats
  status        Show project status dashboard
  health        Machine-readable index health
  query         Search symbols and files (FTS5)
  grep          Raw text search (built-in, rg-like)
  blast-radius  Blast radius of changing a file
  decisions     Recorded decisions
  learn         Add a knowledge note
  warnings      Warnings (fragile files, dead code, large files)
  watch         Watch for changes and re-analyze
  ensure-watch  Init + background watch
  watch-status  Background watcher health

Options:
  -p, --project <PROJECT>  Project root (default: cwd)
      --json               JSON output
  -h, --help               Help
  -V, --version            Version
```

## Mimari

```
ctx-agent/
├── src/
│   ├── main.rs              # entry point
│   ├── cli.rs               # komut tanımları
│   ├── commands/            # komut implementasyonları
│   ├── db/
│   │   ├── mod.rs           # DB core (open/exists)
│   │   ├── dependencies.rs  # dependency persistence + çözümleme
│   │   ├── search.rs        # FTS5 index + sorgu
│   │   ├── decisions.rs     # karar işlemleri
│   │   ├── knowledge.rs     # bilgi notları
│   │   ├── stats.rs         # sağlık + aggregate istatistik
│   │   ├── models.rs        # veri modelleri
│   │   └── schema.rs        # şema migrasyonları
│   ├── analyzer/
│   │   ├── mod.rs           # orkestratör
│   │   ├── scanner.rs       # dosya keşfi + .gitignore
│   │   ├── parser/          # dil başına tree-sitter extractor'lar
│   │   └── graph.rs         # dependency graph + blast radius
│   ├── git/history.rs       # commit analizi + churn skoru
│   ├── query/               # arama + blast radius görünümü
│   └── watcher/             # dosya izleyici
└── mcp-server/              # TypeScript MCP server
```

## Nasıl çalışıyor

1. **Scan** — `.gitignore`'a uyarak dizini gezer, dil tespiti ve dosya hash'i çıkarır
2. **Parse** — tree-sitter ile desteklenen dillerden sembol/import çıkarır
3. **Store** — Her şey proje başına bir SQLite dosyasına (WAL mode)
4. **Index** — Semboller FTS5 virtual table'a yazılır, arama anında
5. **Analyze** — Git geçmişinden churn skoru ve karar çıkarımı
6. **Serve** — CLI ya da MCP üzerinden agent'a sunulur

## Tasarım prensipleri

- Local-first: veri makinende kalır
- Offline: internet, API key, hesap yok
- Incremental: hash bazlı, sadece değişen dosya yeniden analiz edilir
- Tek binary: runtime bağımlılığı yok
- Agent-native: MCP birinci sınıf vatandaş
- `--json` her yerde

## Gerçek proje validasyonu

Production polyglot bir monorepo'da (TypeScript + Go + Swift, 483 dosya,
77k satır) MCP üzerinden test edildi; detaylar:

- [docs/REAL_WORLD_MCP_VALIDATION.md](docs/REAL_WORLD_MCP_VALIDATION.md)
- [docs/AGENT_WORKFLOW.md](docs/AGENT_WORKFLOW.md)

## Katkı

[CONTRIBUTING.md](CONTRIBUTING.md)'ye bak. Güvenlik bulguları için
[SECURITY.md](SECURITY.md).

## Lisans

MIT
