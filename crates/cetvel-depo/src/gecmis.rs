use std::path::Path;

use cetvel_model::{AnlikV2, KaynakTuru, UtcZaman};
use rusqlite::{Connection, OptionalExtension, params};

use crate::DepoHatasi;
use crate::atomik::kullanici_dacl_uygula;

/// Veritabanından okunan tek bir pencere örneği.
#[derive(Debug, Clone, PartialEq)]
pub struct GecmisOrnegi {
    pub havuz: String,
    pub pencere: String,
    pub zaman: UtcZaman,
    pub kullanilan: f64,
    pub kalan: f64,
    pub sifirlama: Option<UtcZaman>,
    pub kaynak: String,
    pub hassasiyet: Option<f64>,
}

/// SQLite (WAL) tabanlı yerel okuma geçmişi ve bildirim olay deposu.
pub struct GecmisDeposu {
    conn: Connection,
}

impl GecmisDeposu {
    /// Verilen dosya yolunda `gecmis.sqlite` veritabanını WAL modunda açar ve tabloları hazırlar.
    pub fn ac(yol: &Path) -> Result<Self, DepoHatasi> {
        if let Some(ebeveyn) = yol.parent() {
            std::fs::create_dir_all(ebeveyn)?;
            let _ = kullanici_dacl_uygula(ebeveyn);
        }
        let conn = Connection::open(yol)?;
        let depo = Self { conn };
        depo.hazirla(true)?;
        let _ = kullanici_dacl_uygula(yol);
        Ok(depo)
    }

    /// Birim testleri için bellek içi SQLite deposu açar.
    pub fn bellekte() -> Result<Self, DepoHatasi> {
        let conn = Connection::open_in_memory()?;
        let depo = Self { conn };
        depo.hazirla(false)?;
        Ok(depo)
    }

    fn hazirla(&self, wal_etkin: bool) -> Result<(), DepoHatasi> {
        if wal_etkin {
            self.conn.pragma_update(None, "journal_mode", "WAL")?;
            self.conn.pragma_update(None, "synchronous", "NORMAL")?;
        }
        self.conn.pragma_update(None, "busy_timeout", 5000)?;
        // WAL dosyası büyüyüp sabit kalmasın: kontrol noktasından sonra en çok 1 MiB tutulur.
        self.conn
            .pragma_update(None, "journal_size_limit", 1_048_576)?;
        // Negatif değer KiB'dir: bağlantı başına 512 KiB sayfa önbelleği.
        self.conn.pragma_update(None, "cache_size", -512)?;
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS ornekler (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                havuz TEXT NOT NULL,
                pencere TEXT NOT NULL,
                zaman INTEGER NOT NULL,
                kullanilan REAL NOT NULL,
                kalan REAL NOT NULL,
                sifirlama INTEGER,
                kaynak TEXT NOT NULL,
                hassasiyet REAL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_ornekler_tekil
                ON ornekler (havuz, pencere, zaman);
            CREATE INDEX IF NOT EXISTS idx_ornekler_sorgu
                ON ornekler (havuz, pencere, zaman DESC);

            CREATE TABLE IF NOT EXISTS olaylar (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                zaman INTEGER NOT NULL,
                tur TEXT NOT NULL,
                havuz TEXT,
                pencere TEXT,
                anahtar TEXT,
                detay TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_olaylar_anahtar
                ON olaylar (anahtar, zaman DESC);
            "#,
        )?;
        Ok(())
    }

    /// Anlık v2 belgesindeki tüm havuz pencerelerini tek işlemde kaydeder.
    ///
    /// Yalnız gerçek ölçümler geçmişe girer: kaynağı `hesap` olan (sıfırlama saati geçince
    /// sentetik `%0` yapılmış) havuzlar yazılmaz. Örnek anahtarı `(havuz, pencere, kaynak.okuma)`
    /// olduğundan hesaplanmış değer aynı zaman damgalı gerçek son okumayı ezerdi.
    pub fn anlik_kaydet(&mut self, anlik: &AnlikV2) -> Result<usize, DepoHatasi> {
        let tx = self.conn.transaction()?;
        let mut eklenen = 0usize;

        for (havuz_id, havuz) in &anlik.havuzlar {
            if havuz
                .kaynak
                .as_ref()
                .is_some_and(|k| k.tur == KaynakTuru::Hesap)
            {
                continue;
            }
            let okuma_zamani = havuz
                .kaynak
                .as_ref()
                .map(|k| k.okuma)
                .unwrap_or(anlik.zaman)
                .as_offset()
                .unix_timestamp();
            let kaynak_str = havuz
                .kaynak
                .as_ref()
                .map(|k| kaynak_turu_metin(k.tur))
                .unwrap_or("bilinmiyor");
            let hassasiyet = havuz.kaynak.as_ref().and_then(|k| k.hassasiyet);

            for (pencere_ad, olcum) in &havuz.pencereler {
                let sifirlama_ts = olcum.sifirlama.map(|z| z.as_offset().unix_timestamp());
                let satir = tx.execute(
                    r#"
                    INSERT INTO ornekler (havuz, pencere, zaman, kullanilan, kalan, sifirlama, kaynak, hassasiyet)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    ON CONFLICT(havuz, pencere, zaman) DO UPDATE SET
                        kullanilan = excluded.kullanilan,
                        kalan = excluded.kalan,
                        sifirlama = excluded.sifirlama,
                        kaynak = excluded.kaynak,
                        hassasiyet = excluded.hassasiyet
                    WHERE excluded.kaynak <> 'hesap'
                    "#,
                    params![
                        havuz_id,
                        pencere_ad,
                        okuma_zamani,
                        olcum.kullanilan,
                        olcum.kalan,
                        sifirlama_ts,
                        kaynak_str,
                        hassasiyet
                    ],
                )?;
                eklenen += satir;
            }
        }

        tx.commit()?;
        Ok(eklenen)
    }

    /// `[baslangic, bitis]` aralığını en fazla `en_fazla` eşit dilime bölüp her dilimin en yüksek
    /// kullanımlı örneğini kronolojik sırada döndürür. "Son N örnek" yerine tüm aralığı kapsar:
    /// sık yazılan (statusline) havuzda 30 günlük grafik son birkaç saate daralmaz.
    pub fn pencere_ornekleri_seyrek(
        &self,
        havuz: &str,
        pencere: &str,
        baslangic: UtcZaman,
        bitis: UtcZaman,
        en_fazla: usize,
    ) -> Result<Vec<GecmisOrnegi>, DepoHatasi> {
        let dilim_sayisi = en_fazla.clamp(1, 10_000) as i64;
        let baslangic_ts = baslangic.as_offset().unix_timestamp();
        let aralik = bitis.fark_sn(baslangic).max(1);
        let dilim_sn = aralik / dilim_sayisi + 1;

        // SQLite: tek MAX() toplamında çıplak sütunlar en yüksek satırdan gelir.
        let mut stmt = self.conn.prepare(
            r#"
            SELECT havuz, pencere, zaman, MAX(kullanilan), kalan, sifirlama, kaynak, hassasiyet
            FROM ornekler
            WHERE havuz = ?1 AND pencere = ?2 AND zaman >= ?3
            GROUP BY (zaman - ?3) / ?4
            ORDER BY zaman ASC
            LIMIT ?5
            "#,
        )?;
        let rows = stmt.query_map(
            params![havuz, pencere, baslangic_ts, dilim_sn, dilim_sayisi],
            |row| {
                let zaman_ts: i64 = row.get(2)?;
                let sifirlama_ts: Option<i64> = row.get(5)?;
                Ok(GecmisOrnegi {
                    havuz: row.get(0)?,
                    pencere: row.get(1)?,
                    zaman: UtcZaman::from_unix_timestamp(zaman_ts).unwrap_or_else(UtcZaman::simdi),
                    kullanilan: row.get(3)?,
                    kalan: row.get(4)?,
                    sifirlama: sifirlama_ts.and_then(UtcZaman::from_unix_timestamp),
                    kaynak: row.get(6)?,
                    hassasiyet: row.get(7)?,
                })
            },
        )?;
        let mut sonuc = Vec::new();
        for item in rows {
            sonuc.push(item?);
        }
        Ok(sonuc)
    }

    /// Belirli bir havuz ve pencerenin son örneklerini kronolojik artan sırada döndürür (`limit <= 10_000`).
    pub fn pencere_ornekleri(
        &self,
        havuz: &str,
        pencere: &str,
        baslangic: UtcZaman,
        limit: usize,
    ) -> Result<Vec<GecmisOrnegi>, DepoHatasi> {
        let sinir = limit.clamp(1, 10_000) as i64;
        let baslangic_ts = baslangic.as_offset().unix_timestamp();

        let mut stmt = self.conn.prepare(
            r#"
            SELECT havuz, pencere, zaman, kullanilan, kalan, sifirlama, kaynak, hassasiyet
            FROM (
                SELECT havuz, pencere, zaman, kullanilan, kalan, sifirlama, kaynak, hassasiyet
                FROM ornekler
                WHERE havuz = ?1 AND pencere = ?2 AND zaman >= ?3
                ORDER BY zaman DESC
                LIMIT ?4
            )
            ORDER BY zaman ASC
            "#,
        )?;

        let rows = stmt.query_map(params![havuz, pencere, baslangic_ts, sinir], |row| {
            let zaman_ts: i64 = row.get(2)?;
            let sifirlama_ts: Option<i64> = row.get(5)?;
            Ok(GecmisOrnegi {
                havuz: row.get(0)?,
                pencere: row.get(1)?,
                zaman: UtcZaman::from_unix_timestamp(zaman_ts).unwrap_or_else(UtcZaman::simdi),
                kullanilan: row.get(3)?,
                kalan: row.get(4)?,
                sifirlama: sifirlama_ts.and_then(UtcZaman::from_unix_timestamp),
                kaynak: row.get(6)?,
                hassasiyet: row.get(7)?,
            })
        })?;

        let mut sonuc = Vec::new();
        for item in rows {
            sonuc.push(item?);
        }
        Ok(sonuc)
    }

    /// Tekilleştirme anahtarının olay tablosunda kayıtlı olup olmadığını kontrol eder.
    pub fn olay_var_mi(&self, anahtar: &str) -> Result<bool, DepoHatasi> {
        let bulundu: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM olaylar WHERE anahtar = ?1 LIMIT 1",
                params![anahtar],
                |row| row.get(0),
            )
            .optional()?;
        Ok(bulundu.is_some())
    }

    /// Belirtilen tekilleştirme anahtarlı olayı kaydeder.
    pub fn olay_kaydet(
        &self,
        zaman: UtcZaman,
        tur: &str,
        havuz: Option<&str>,
        pencere: Option<&str>,
        anahtar: Option<&str>,
        detay: Option<&str>,
    ) -> Result<(), DepoHatasi> {
        self.conn.execute(
            r#"
            INSERT INTO olaylar (zaman, tur, havuz, pencere, anahtar, detay)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                zaman.as_offset().unix_timestamp(),
                tur,
                havuz,
                pencere,
                anahtar,
                detay
            ],
        )?;
        Ok(())
    }

    /// Son 24 saatte belirtilen tür ve havuz için kaç olay kaydedildiğini döndürür.
    pub fn son_24_saat_olay_sayisi(
        &self,
        tur: &str,
        havuz: &str,
        simdi: UtcZaman,
    ) -> Result<u64, DepoHatasi> {
        let sinir = simdi.ekle_sn(-86400).as_offset().unix_timestamp();
        let sayi: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM olaylar WHERE tur = ?1 AND havuz = ?2 AND zaman >= ?3",
            params![tur, havuz, sinir],
            |row| row.get(0),
        )?;
        Ok(sayi.max(0) as u64)
    }

    /// `saklama_gun` günden eski ham örnekleri temizler (90 gün).
    pub fn eski_verileri_temizle(
        &self,
        simdi: UtcZaman,
        saklama_gun: u64,
    ) -> Result<usize, DepoHatasi> {
        let kesim = simdi
            .ekle_sn(-((saklama_gun as i64) * 86400))
            .as_offset()
            .unix_timestamp();
        let silinen = self
            .conn
            .execute("DELETE FROM ornekler WHERE zaman < ?1", params![kesim])?;
        // Bildirim tekilleştirme kayıtları da süresiz büyümesin.
        let _ = self
            .conn
            .execute("DELETE FROM olaylar WHERE zaman < ?1", params![kesim])?;
        // WAL'ı ana dosyaya işleyip kes: günde bir çalıştığı için maliyeti önemsizdir.
        let _ = self
            .conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
        Ok(silinen)
    }
}

fn kaynak_turu_metin(tur: KaynakTuru) -> &'static str {
    match tur {
        KaynakTuru::ModKoprusu => "mod_koprusu",
        KaynakTuru::Statusline => "statusline",
        KaynakTuru::CodexAppServer => "codex_app_server",
        KaynakTuru::RolloutYedek => "rollout_yedek",
        KaynakTuru::AgyUsage => "agy_usage",
        KaynakTuru::ResmiApi => "resmi_api",
        KaynakTuru::BakiyeApi => "bakiye_api",
        KaynakTuru::Hesap => "hesap",
        KaynakTuru::YerelGunluk => "yerel_gunluk",
        KaynakTuru::Bilinmiyor => "bilinmiyor",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use cetvel_model::{Durum, GuvenEtiketi, HavuzAnlik, KaynakBilgisi, PencereOlcum};

    fn z(s: &str) -> UtcZaman {
        UtcZaman::parse_tarih_veya_zaman(s).expect("zaman")
    }

    fn ornek_anlik(okuma: UtcZaman, kullanilan: f64) -> AnlikV2 {
        let mut pencereler = BTreeMap::new();
        pencereler.insert(
            "5s".to_string(),
            PencereOlcum {
                uzunluk_dk: 300,
                kullanilan,
                kalan: 1.0 - kullanilan,
                sifirlama: Some(z("2026-11-15T23:00:00Z")),
                etiket: None,
            },
        );
        let mut havuz = HavuzAnlik {
            platform: "claude".into(),
            etiket: "Claude".into(),
            durum: Durum::Ok,
            etkin_kalan: None,
            baglayici_pencere: None,
            kullanilabilir: None,
            kaynak: Some(KaynakBilgisi {
                tur: KaynakTuru::ModKoprusu,
                guven: GuvenEtiketi::ResmiArayuz,
                okuma,
                yas_sn: 0,
                hassasiyet: Some(0.1),
            }),
            pencereler,
            alt_sayaclar: BTreeMap::new(),
            tahmin: BTreeMap::new(),
            baski: None,
            bakiye: None,
            ek: None,
            hata: None,
            notlar: Vec::new(),
            kesif: None,
            etkin_kalan_zincir: None,
        };
        havuz.hesapla_etkin_durum();
        let mut havuzlar = BTreeMap::new();
        havuzlar.insert("claude_max".to_string(), havuz);
        AnlikV2::yeni(okuma, 1, havuzlar)
    }

    #[test]
    fn sqlite_wal_dosyada_kayit_ve_sorgu_calisir() {
        let kok = std::env::temp_dir().join(format!("cetvel-sqlite-test-{}", std::process::id()));
        let db_yol = kok.join("gecmis.sqlite");
        {
            let mut depo = GecmisDeposu::ac(&db_yol).expect("ac");
            let t0 = z("2026-11-15T18:00:00Z");
            depo.anlik_kaydet(&ornek_anlik(t0, 0.10)).expect("kayit1");
            depo.anlik_kaydet(&ornek_anlik(t0.ekle_sn(600), 0.25))
                .expect("kayit2");
            depo.anlik_kaydet(&ornek_anlik(t0.ekle_sn(1200), 0.40))
                .expect("kayit3");

            let liste = depo
                .pencere_ornekleri("claude_max", "5s", t0, 100)
                .expect("sorgu");
            assert_eq!(liste.len(), 3);
            // Seyrek sorgu tüm aralığı kapsar; dilim başına en yüksek kullanım.
            let seyrek = depo
                .pencere_ornekleri_seyrek("claude_max", "5s", t0, t0.ekle_sn(1200), 2)
                .expect("seyrek");
            assert_eq!(seyrek.len(), 2);
            assert!((seyrek[0].kullanilan - 0.25).abs() < 1e-9);
            assert!((seyrek[1].kullanilan - 0.40).abs() < 1e-9);
            assert!((liste[0].kullanilan - 0.10).abs() < 1e-9);
            assert!((liste[2].kullanilan - 0.40).abs() < 1e-9);
            assert_eq!(liste[0].hassasiyet, Some(0.1));

            depo.olay_kaydet(
                t0,
                "esik_gecisi",
                Some("claude_max"),
                Some("5s"),
                Some("claude_max/5s/2026-11-15T23:00:00Z"),
                Some("test"),
            )
            .expect("olay");
            assert!(
                depo.olay_var_mi("claude_max/5s/2026-11-15T23:00:00Z")
                    .expect("var mi")
            );
        }
        let _ = std::fs::remove_dir_all(&kok);
    }

    #[test]
    fn hesaplanmis_havuz_gercek_son_okumayi_ezmez() {
        let mut depo = GecmisDeposu::bellekte().expect("bellek");
        let t0 = z("2026-11-15T22:00:00Z");
        let gercek = ornek_anlik(t0, 0.90);
        depo.anlik_kaydet(&gercek).expect("gercek");

        // Sıfırlama geçince aynı `okuma` damgasıyla sentetik %0 (kaynak = hesap) yayınlanır.
        let mut hesap = gercek.clone();
        let h = hesap.havuzlar.get_mut("claude_max").unwrap();
        let p = h.pencereler.get_mut("5s").unwrap();
        p.kullanilan = 0.0;
        p.kalan = 1.0;
        p.sifirlama = None;
        h.kaynak.as_mut().unwrap().tur = KaynakTuru::Hesap;
        assert_eq!(depo.anlik_kaydet(&hesap).expect("hesap"), 0);

        let liste = depo
            .pencere_ornekleri("claude_max", "5s", t0.ekle_sn(-60), 10)
            .expect("sorgu");
        assert_eq!(liste.len(), 1);
        assert!((liste[0].kullanilan - 0.90).abs() < 1e-9);
        assert_eq!(liste[0].kaynak, "mod_koprusu");
        assert!(liste[0].sifirlama.is_some());
    }

    #[test]
    fn eski_olaylar_da_temizlenir() {
        let depo = GecmisDeposu::bellekte().expect("bellek");
        let yeni = z("2026-11-15T18:00:00Z");
        depo.olay_kaydet(
            z("2026-01-01T00:00:00Z"),
            "esik",
            None,
            None,
            Some("eski"),
            None,
        )
        .unwrap();
        depo.olay_kaydet(yeni, "esik", None, None, Some("yeni"), None)
            .unwrap();
        depo.eski_verileri_temizle(yeni, 90).unwrap();
        assert!(!depo.olay_var_mi("eski").unwrap());
        assert!(depo.olay_var_mi("yeni").unwrap());
    }

    #[test]
    fn eski_veriler_temizlenir() {
        let mut depo = GecmisDeposu::bellekte().expect("bellek");
        let eski = z("2026-01-01T00:00:00Z");
        let yeni = z("2026-11-15T18:00:00Z");
        depo.anlik_kaydet(&ornek_anlik(eski, 0.2)).expect("eski");
        depo.anlik_kaydet(&ornek_anlik(yeni, 0.3)).expect("yeni");

        let silinen = depo.eski_verileri_temizle(yeni, 90).expect("temizle");
        assert_eq!(silinen, 1);

        let kalan = depo
            .pencere_ornekleri("claude_max", "5s", eski, 100)
            .expect("sorgu");
        assert_eq!(kalan.len(), 1);
        assert_eq!(kalan[0].zaman, yeni);
    }
}
