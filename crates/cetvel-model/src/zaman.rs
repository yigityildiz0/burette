use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{Date, Month, OffsetDateTime, Time, UtcOffset};

/// Her zaman UTC olarak tutulan ve sonu `Z` ile saniye çözünürlüğünde serileştirilen zaman damgası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, JsonSchema)]
pub struct UtcZaman(#[schemars(with = "String")] OffsetDateTime);

impl UtcZaman {
    /// Geçerli sistem zamanını UTC olarak döndürür.
    pub fn simdi() -> Self {
        Self::from_offset(OffsetDateTime::now_utc())
    }

    /// Verilen `OffsetDateTime` değerini UTC'ye çevirir ve nanosaniyeyi sıfırlar.
    pub fn from_offset(dt: OffsetDateTime) -> Self {
        let utc = dt.to_offset(UtcOffset::UTC);
        let truncated = utc.replace_nanosecond(0).unwrap_or(utc);
        Self(truncated)
    }

    /// Unix epoch saniyesinden UTC zaman üretir.
    pub fn from_unix_timestamp(sec: i64) -> Option<Self> {
        OffsetDateTime::from_unix_timestamp(sec)
            .ok()
            .map(Self::from_offset)
    }

    /// `YYYY-MM-DD` tarihini ya da tam RFC3339 / ISO-8601 metnini UTC zamanına çevirir.
    pub fn parse_tarih_veya_zaman(girdi: &str) -> Result<Self, time::error::Parse> {
        let temiz = girdi.trim();
        if let Ok(dt) = OffsetDateTime::parse(temiz, &Rfc3339) {
            return Ok(Self::from_offset(dt));
        }
        // Bare YYYY-MM-DD desteği (ör. kampanya_bitis = "2026-12-01" -> 00:00:00Z).
        if temiz.len() == 10
            && temiz.as_bytes()[4] == b'-'
            && temiz.as_bytes()[7] == b'-'
            && let (Ok(yil), Ok(ay_u8), Ok(gun)) = (
                temiz[0..4].parse::<i32>(),
                temiz[5..7].parse::<u8>(),
                temiz[8..10].parse::<u8>(),
            )
            && let Ok(ay) = Month::try_from(ay_u8)
            && let Ok(tarih) = Date::from_calendar_date(yil, ay, gun)
        {
            let dt = OffsetDateTime::new_utc(tarih, Time::MIDNIGHT);
            return Ok(Self(dt));
        }
        OffsetDateTime::parse(temiz, &Rfc3339).map(Self::from_offset)
    }

    /// İçteki `OffsetDateTime` değerini döndürür.
    pub fn as_offset(&self) -> OffsetDateTime {
        self.0
    }

    /// İki zaman arasındaki saniye farkını (`self - onceki`) döndürür.
    pub fn fark_sn(&self, onceki: Self) -> i64 {
        (self.0 - onceki.0).whole_seconds()
    }

    /// `saniye` ekleyerek yeni zaman döndürür. Taşmada panik atmaz, desteklenen sınıra doyar
    /// (girdiden gelen aşırı süreler okuyucuyu düşürmesin).
    pub fn ekle_sn(&self, saniye: i64) -> Self {
        Self(self.0.saturating_add(time::Duration::seconds(saniye)))
    }

    /// Bir sonraki UTC gece yarısını (`00:00:00Z`) döndürür.
    pub fn sonraki_utc_gece_yarisi(&self) -> Self {
        let sonraki_gun = self.0.date().next_day().unwrap_or(self.0.date());
        Self(OffsetDateTime::new_utc(sonraki_gun, Time::MIDNIGHT))
    }

    /// Sonu `Z` ile biten ISO-8601 metnine çevirir.
    pub fn to_iso_z(&self) -> String {
        let fmt = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
        self.0
            .format(&fmt)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
    }
}

impl std::fmt::Display for UtcZaman {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_iso_z())
    }
}

impl Serialize for UtcZaman {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_iso_z())
    }
}

impl<'de> Deserialize<'de> for UtcZaman {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let metin = String::deserialize(deserializer)?;
        Self::parse_tarih_veya_zaman(&metin).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_zaman_z_ile_serilestirir() {
        let z = UtcZaman::parse_tarih_veya_zaman("2026-11-15T21:30:05+03:00").expect("zaman");
        assert_eq!(z.to_iso_z(), "2026-11-15T18:30:05Z");
        let json = serde_json::to_string(&z).expect("json");
        assert_eq!(json, "\"2026-11-15T18:30:05Z\"");
    }

    #[test]
    fn ekle_sn_tasmada_panik_atmaz() {
        let z = UtcZaman::parse_tarih_veya_zaman("2026-11-16T10:00:00Z").expect("zaman");
        let geri = z.ekle_sn(-1_000_000_000_000_000_000);
        assert!(geri < z);
        let ileri = z.ekle_sn(i64::MAX);
        assert!(ileri > z);
        assert_eq!(z.ekle_sn(60).fark_sn(z), 60);
    }

    #[test]
    fn ciplak_tarih_utc_gece_yarisi_olarak_cozulur() {
        let z = UtcZaman::parse_tarih_veya_zaman("2026-12-01").expect("tarih");
        assert_eq!(z.to_iso_z(), "2026-12-01T00:00:00Z");
    }
}
