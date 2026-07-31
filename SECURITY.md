# Güvenlik

## Desteklenen sürümler

Sadece `main` dalının güncel hali destekleniyor.

## Güvenlik açığı bildirimi

Lütfen herkese açık issue açma. GitHub Security Advisories üzerinden özel olarak
bildir; o mümkün değilse maintainer'a özel kanaldan ulaş.

Bildirimde şunlar olsun:

- Etkilenen versiyon/commit
- Nasıl tekrarlanır
- Etkisi ne
- Varsa fix önerisi

72 saat içinde dönüş yapmaya çalışıyorum.

## Kapsam

Kapsam içi:

- `ctx` CLI
- MCP server (`mcp-server/`)
- Lokal SQLite'ta tutulan veriler

Kapsam dışı:

- Projeye özel exploit yolu olmayan üçüncü parti paket açıkları
- Dış sistemlerin yanlış konfigürasyonu
