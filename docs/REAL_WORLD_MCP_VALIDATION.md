# Gerçek proje validasyonu (MCP)

Test ettiğim proje: production'da çalışan polyglot bir monorepo
(TypeScript + Go + Swift).

Sadece MCP araçları kullanıldı: `ctx_init`, `ctx_status`, `ctx_scan`, `ctx_query`,
`ctx_blast_radius`, `ctx_warnings`, `ctx_decisions`.

## Projenin boyutu

- 483 dosya, ~77.235 satır
- 3.169 sembol, 1.428 dependency
- 171 karar (git geçmişinden)

## Sonuçlar

`ctx_init`: proje zaten initialize'lıydı, re-scan ~3.7s sürdü (483 dosyanın 0'ı
değişmişti — incremental çalışıyor).

`ctx_status`: dil dağılımı doğru geldi — TypeScript 260, Go 131, JSON 24,
Swift 23, Markdown 14 dosya.

`ctx_query "auth"`: 50 sonuç (üst limite takıldı). Backend auth helper'ları, JWT
yolları, telegram auth store, panel auth context ilk sayfada.

`ctx_query "payment"`: 28 sonuç, payment UI akışı ve formlar ilk sayfada.

`ctx_warnings`: 13 uyarı, hepsi büyük dosya (>500 satır). Bu koşuda kırılgan/ölü
dosya uyarısı çıkmadı.

`ctx_blast_radius apps/core/internal/admin/handler.go`: 19 import, 0 imported-by
→ düşük risk.

`ctx_decisions`: 171 karar. Commit geçmişinden mimari bağlamı çıkarmak düşündüğümden
hızlı çalıştı.

## Agent açısından anlamı

Yeni bir oturumda agent birkaç dakika içinde şunları öğreniyor:

- mimarinin kabaca şekli,
- işle ilgili kod bölgeleri,
- edit öncesi riskli/büyük dosyalar,
- geçmişte verilmiş kararlar (commit'lerden).

Kör gezinti azalıyor, ilk denemede doğru yere edit yapma şansı artıyor.

## Bildiğim sınırlar

- Blast radius kalitesi, dilin import çıkarma kalitesine bağlı. Sembol çıkarılmayan
  dillerde graph eksik kalır.
- Query çıktısı üstten kırpılıyor; tek geniş sorgu yerine birkaç odaklı sorgu
  atmak lazım.
- Sağlık sinyalleri yapısal; test/çalıştırma yerine geçmez.
