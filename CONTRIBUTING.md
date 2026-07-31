# Katkı

Önce issue aç, neyi değiştirmek istediğini konuşalım. Büyük değişikliklerde bu
özellikle önemli — boşa emek harcamayalım.

## Geliştirme ortamı

Gerekenler:

- Rust (stable)
- Node.js 18+ (sadece MCP server için)

```bash
cargo build
cd mcp-server && npm install && npm run build
```

## Dal ve commit düzeni

- `main`'den feature dalı aç.
- Commit'leri küçük ve odaklı tut.
- Commit mesajında ne yaptığın ve neden yaptığın belli olsun; Conventional
  Commits (`feat:`, `fix:` vs.) kullanırsan `ctx decisions` bunları karar olarak
  da topluyor, güzel oluyor.

## PR açmadan önce

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cd mcp-server && npm run build
```

Davranış değiştiriyorsan ilgili dokümanı da güncelle (README veya docs/).

## Genel prensipler

- Değişiklik minimal ve hedefli olsun.
- Agent'a dönen çıktılar deterministik kalsın (`--json` kırılmasın).
- CLI/MCP'de geriye dönük kırılma yapacaksan PR açıklamasında belirt.
- Secret, token, credential hiçbir şekilde repoya girmesin.

## Issue açarken

Bug bildiriminde şunlar olsun: ne yaptın, ne bekliyordun, ne oldu, ortam bilgisi
(OS, Rust versiyonu, `ctx --version`).
