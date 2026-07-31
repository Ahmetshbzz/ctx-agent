# Agent workflow: ctx-agent ile çalışma düzeni

Amaç basit: agent oturumu dosya gezerek değil, ctx-agent'ın hazırladığı bağlamla
başlasın. Böylece ilk dakikalardaki kör gezinti ortadan kalkıyor.

## Oturum başlangıcı

1. `ctx_status` — proje özeti. İlk çalıştırmada (hiç bilgi notu yoksa) bir
   overview notu otomatik oluşur.
2. `ctx_warnings` — büyük/kırılgan/ölü dosyalar. Edit yapmadan önce risk
   haritası burada.
3. 2-3 tane odaklı `ctx_query` — tek geniş sorgu yerine domain bazlı terimler
   (`auth`, `session`, `payment` gibi). Sembol araması boş dönerse kendisi metin
   aramasına düşüyor.

Yapı ağırlıklı işlerde `ctx_map` de faydalı ama her seferinde şart değil.

## Edit öncesi

- Değiştirmeyi düşündüğün dosyaya `ctx_blast_radius` çalıştır. Kimler import
  ediyor gör, ona göre davran.
- auth/session/token/crypto tarafına dokunuyorsan `ctx_guard` çalıştır;
  BLOCK dönerse eksik kontrol var demektir.

## Edit sonrası

1. `ctx_scan` — incremental yeniden analiz (sadece değişenler).
2. Gerekirse tekrar `ctx_query` / `ctx_blast_radius` ile doğrula.
3. Bariz olmayan bir mimari karar verdiysen `ctx_learn` ile not düş. Bir sonraki
   oturumda agent bunu görecek.

## Deneyimlerim

- `ctx_query` çok sonuç döndürüyorsa terimi daralt; domain kelimesi ekle.
- `ctx_warnings`'ta büyük dosya varsa yeni feature'dan önce o dosyayı bölmek
  genelde daha ucuz oluyor.
- `ctx_decisions` doluysa önce ona bak — daha önce reddedilmiş bir pattern'i
  tekrar önermek gereksiz.

## CLI karşılıkları

```bash
ctx -p /path/to/project status --json
ctx -p /path/to/project warnings --json
ctx -p /path/to/project query "auth" --json
ctx -p /path/to/project blast-radius src/db/mod.rs --json
ctx -p /path/to/project scan --json
```
