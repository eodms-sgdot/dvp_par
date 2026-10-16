use chrono::{NaiveDate, NaiveTime};

// Code adapted from
// https://github.com/Direction-des-inventaires-forestiers/StereoPhoto/blob/13fe0616e998ae6215703e07612f0fcf0353ba95/src/worldManager.py

#[derive(Clone, Debug, Default)]
pub struct PAR {
    pub global: Global,
    pub interior_orientation: Interior,
    pub camera_calibration: Option<CameraCalibration>,
    pub exterior_orientation: Exterior,
    pub ground_pixel_size: f64,
    pub rotation_matrix: [[f64; 3]; 3],
}

#[derive(Clone, Debug, Default)]
pub struct Global {
    pub date: NaiveDate,
    pub time: NaiveTime,
    pub scale: f64,
    pub file: String,
    pub image_width: u32,
    pub image_height: u32,
    pub ext_type: String,
}

#[derive(Clone, Debug, Default)]
pub struct Interior {
    pub date: NaiveDate,
    pub time: NaiveTime,
    pub affine: [f64; 6],
    pub inverse_affine: [f64; 6],
    pub focal_length: f64,
}

#[derive(Clone, Debug, Default)]
pub struct CameraCalibration {
    pub name: String,
    pub date: Option<NaiveDate>,
    pub focal_length: f64,
    pub principal_point: (f64, f64),
    pub pixel_size: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Exterior {
    pub date: NaiveDate,
    pub time: NaiveTime,
    pub xyz: XYZ,
    pub opk: OPK,
}

// Camera position in 3D space
#[derive(Clone, Debug, Default)]
pub struct XYZ {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

// Camera orientation in 3D space (omega, phi, kappa)
#[derive(Clone, Debug, Default)]
pub struct OPK {
    pub omega: f64,
    pub phi: f64,
    pub kappa: f64,
}

#[derive(Clone, Debug)]
pub struct Pixel {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug)]
pub struct Coord {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug)]
pub enum PARError {
    DateTimeParsing(String),
    InvalidLine(String),
    InvalidScale,
    InvalidImageWidth,
    InvalidImageHeight,
    InvalidParaffine,
    InvalidParinvaff,
    InvalidFocalLength,
    InvalidXYZ,
    InvalidOPK,
}

impl From<chrono::ParseError> for PARError {
    fn from(e: chrono::ParseError) -> Self {
        Self::DateTimeParsing(e.to_string())
    }
}

impl TryFrom<&str> for PAR {
    type Error = PARError;
    #[allow(clippy::too_many_lines)]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let mut par = Self::default();

        let mut current_section = String::new();
        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('.') {
                current_section = line.to_string();
                continue;
            }
            let Some((key, mut value)) = line.split_once(' ') else {
                continue;
            };
            value = value.trim();
            tracing::debug!("section: {current_section}, key: {key}, value: {value}");
            match current_section.as_str() {
                ".GLOBAL" => match key {
                    "$DATE" => par.global.date = parse_date(value)?,
                    "$TIME" => par.global.time = value.parse()?,
                    "$FSCALE00" => par.global.scale = value.parse().map_err(|_| PARError::InvalidScale)?,
                    "$FILE00" => {
                        let value = value.replace('\\', "/");
                        let basename = std::path::Path::new(&value)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .ok_or_else(|| PARError::InvalidLine(line.to_string()))?;

                        par.global.file = basename.to_string();
                    }
                    "$FWIDTH00" => {
                        par.global.image_width = value.parse().map_err(|_| PARError::InvalidImageWidth)?;
                    }
                    "$FHEIGHT00" => {
                        par.global.image_height = value.parse().map_err(|_| PARError::InvalidImageHeight)?;
                    }
                    "$TYPE" => par.global.ext_type = value.to_string(),
                    _ => {
                        tracing::warn!("Unknown key \"{key}\" in .GLOBAL");
                    }
                },
                ".INTERIOR" => match key {
                    "$DATE" => par.interior_orientation.date = parse_date(value)?,
                    "$TIME" => par.interior_orientation.time = value.parse()?,
                    "$PARAFFINE00" => {
                        let values: Vec<f64> =
                            value.split_whitespace().map(|v| v.parse::<f64>().unwrap()).collect();
                        if values.len() != 6 {
                            return Err(PARError::InvalidParaffine);
                        }
                        par.interior_orientation.affine.copy_from_slice(&values);
                    }
                    "$PARINVAFF00" => {
                        let values: Vec<f64> =
                            value.split_whitespace().map(|v| v.parse::<f64>().unwrap()).collect();
                        if values.len() != 6 {
                            return Err(PARError::InvalidParinvaff);
                        }
                        par.interior_orientation.inverse_affine.copy_from_slice(&values);
                    }
                    "$FOC00" => {
                        par.interior_orientation.focal_length =
                            value.parse().map_err(|_| PARError::InvalidFocalLength)?;
                    }
                    _ => {
                        tracing::warn!("Unknown key \"{key}\" in .INTERIOR");
                    }
                },
                ".CAMCOR00" => {
                    if par.camera_calibration.is_none() {
                        par.camera_calibration = Some(CameraCalibration::default());
                    }
                    let calib = par.camera_calibration.as_mut().unwrap();
                    match key {
                        "$NAME" => calib.name = value.to_string(),
                        "$DATE" => calib.date = Some(parse_date(value)?),
                        "$FOCAL" => {
                            calib.focal_length = value.parse().map_err(|_| PARError::InvalidFocalLength)?;
                        }
                        "$PPA" | "$PPS" => {
                            let values: Vec<f64> =
                                value.split_whitespace().map(|v| v.parse::<f64>().unwrap()).collect();
                            if values.len() != 2 {
                                return Err(PARError::InvalidLine(line.to_string()));
                            }
                            calib.principal_point = (values[0], values[1]);
                        }
                        "$PIXELSIZE" => {
                            calib.pixel_size =
                                value.parse().map_err(|_| PARError::InvalidLine(line.to_string()))?;
                        }
                        _ => {
                            tracing::warn!("Unknown key \"{key}\" in .CAMCOR00");
                        }
                    }
                }
                ".EXTERIOR" => match key {
                    "$DATE" => par.exterior_orientation.date = parse_date(value)?,
                    "$TIME" => par.exterior_orientation.time = value.parse()?,
                    "$XYZ00" => {
                        let values: Vec<f64> =
                            value.split_whitespace().map(|v| v.parse::<f64>().unwrap()).collect();
                        if values.len() != 3 {
                            return Err(PARError::InvalidXYZ);
                        }
                        par.exterior_orientation.xyz = XYZ { x: values[0], y: values[1], z: values[2] };
                    }
                    "$OPK00" => {
                        let values: Vec<f64> =
                            value.split_whitespace().map(|v| v.parse::<f64>().unwrap()).collect();
                        if values.len() != 3 {
                            return Err(PARError::InvalidOPK);
                        }
                        par.exterior_orientation.opk =
                            OPK { omega: values[0], phi: values[1], kappa: values[2] };
                    }
                    _ => {
                        tracing::warn!("Unknown key \"{key}\" in .EXTERIOR");
                    }
                },
                _ => {
                    tracing::warn!("Unknown section: {}", current_section);
                }
            }
        }
        let pixel_size = if let Some(calib) = &par.camera_calibration {
            calib.pixel_size * 1e-3
        } else {
            par.interior_orientation.affine[0]
        };
        let fscale = if par.global.scale > 0.0 {
            par.global.scale * 1e-3
        } else {
            par.exterior_orientation.xyz.z / par.interior_orientation.focal_length
        };

        par.ground_pixel_size = fscale * pixel_size;
        par.calc_rotation_matrix();
        Ok(par)
    }
}

impl PAR {
    fn calc_rotation_matrix(&mut self) {
        let omega = self.exterior_orientation.opk.omega;
        let phi = self.exterior_orientation.opk.phi;
        let kappa = self.exterior_orientation.opk.kappa;

        self.rotation_matrix[0][0] = phi.cos() * kappa.cos();
        self.rotation_matrix[0][1] = -phi.cos() * kappa.sin();
        self.rotation_matrix[0][2] = phi.sin();
        self.rotation_matrix[1][0] =
            (omega.sin() * phi.sin()).mul_add(kappa.cos(), omega.cos() * kappa.sin());
        self.rotation_matrix[1][1] =
            (omega.sin() * phi.sin()).mul_add(-kappa.sin(), omega.cos() * kappa.cos());
        self.rotation_matrix[1][2] = -omega.sin() * phi.cos();
        self.rotation_matrix[2][0] =
            (omega.cos() * phi.sin()).mul_add(-kappa.cos(), omega.sin() * kappa.sin());
        self.rotation_matrix[2][1] =
            (omega.cos() * phi.sin()).mul_add(kappa.sin(), omega.sin() * kappa.cos());
        self.rotation_matrix[2][2] = omega.cos() * phi.cos();
    }
    #[must_use]
    pub fn affine_equation(&self, pixel: &Pixel) -> Pixel {
        let x_dst = self.interior_orientation.affine[1]
            .mul_add(pixel.y, self.interior_orientation.affine[0] * pixel.x);
        let y_dst = self.interior_orientation.affine[4]
            .mul_add(pixel.y, self.interior_orientation.affine[3] * pixel.x);
        Pixel { x: x_dst, y: y_dst }
    }
    #[must_use]
    pub fn inverse_affine_equation(&self, pixel_dst: &Pixel) -> Pixel {
        let x_pixel = self.interior_orientation.inverse_affine[1]
            .mul_add(pixel_dst.y, self.interior_orientation.inverse_affine[0] * pixel_dst.x);
        let y_pixel = self.interior_orientation.inverse_affine[4]
            .mul_add(pixel_dst.y, self.interior_orientation.inverse_affine[3] * pixel_dst.x);
        Pixel { x: x_pixel, y: y_pixel }
    }
}

fn parse_date(date_str: &str) -> Result<NaiveDate, PARError> {
    let formats = ["%Y-%b-%d", "%d-%m-%Y"];
    for format in &formats {
        if let Ok(date) = NaiveDate::parse_from_str(date_str, format) {
            return Ok(date);
        }
    }
    Err(PARError::DateTimeParsing(format!("Invalid date format: {date_str}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r"
.GLOBAL
$DATE  2025-Oct-23
$TIME  15:43:16
$FSCALE00     2.4516398010E+004
$FILE00  Eastmain_202504033_IMG_15cm_105.tif
$FWIDTH00       17310
$FHEIGHT00       11310
$TYPE OPK
.INTERIOR
$DATE  2025-Oct-23
$TIME  15:43:16
$PARAFFINE00         +6.00000E-003 +0.00000E+000 -5.19300E+001 +0.00000E+000 -6.00000E-003 +3.39300E+001
$PARINVAFF00         +1.66667E+002 +0.00000E+000 +8.65500E+003 +0.00000E+000 -1.66667E+002 +5.65500E+003
$FOC00       100.50000
.EXTERIOR
$XYZ00               +6.7141282300E+005 +5.7916439310E+006 +2.5138980000E+003
$OPK00               +8.7150000000E-002 -2.4618000000E-001 -8.9780210000E+001
$DATE  2025-Oct-23
$TIME  15:43:16
.CALIBRAGE
.CORRECTION
.END
";
    const SAMPLE2: &str = r"
.GLOBAL
$DATE 14-10-2025
$TIME 16:13:52
$DESCR OPK -> DVP ...
$TYPE OPK
$FSCALE00 3.19890000000000e+004
$FILE00 Matimekosh_202504001_IMG_15cm_030.tif
$VERSION 5.5
.DESCR
.INTERIOR
$FCAM00 EAGLE_ROTATION_0deg
$PARAFFINE00 4.60000000000000e-003 0.00000000000000e+000 -5.29207000000000e+001 0.00000000000000e+000 -4.60000000000000e-003 3.40147000000000e+001
$PARINVAFF00 2.17391304347800e+002 0.00000000000000e+000 1.15045000000000e+004 0.00000000000000e+000 -2.17391304347800e+002 7.39450000000000e+003
$DATE 14-10-2025
$TIME 16:13:52
$FOC00 7.98000000000000e+001
.RELATIVE
.ABSOLUE
.RELEVEMENT
.EXTERNE
.CORRECTION
.CALIBRAGE
.TIN
.INDEX
.LISTMODELES
.CORRECTION
.CAMCOR00
$NAME EAGLE_ROTATION_0deg
$DATE 
$TYPE 1
$FOCAL 7.98000000000000e+001
$PPA 1.15045000000000e+004 7.39450000000000e+003
$PPS 1.15045000000000e+004 7.39450000000000e+003
$XP_OFFSET 0.00000000000000e+000
$YP_OFFSET 0.00000000000000e+000
$PIXELSIZE 4.60000000000000e+000
$DISTORTION_TYPE 0
.EXTERIOR
$DATE 14-10-2025
$TIME 16:13:52
$XYZ00 3.50412453330000e+005 6.07507938189000e+006 3.13203698000000e+003
$OPK00 5.81778330067856e-002 -1.46763521794801e-001 3.29698549737713e+002
.END
";

    const SAMPLE3: &str = r"
.GLOBAL
$DATE 29-01-2008
$TIME 08:53:36
$DESCR Intergraph -> DVP ...
$TYPE OPK
$FSCALE00 1.53622904605189e+004
$FILE00 \\Aphrodite\Production\IMAGE\C07-110\A31926_112.TIF
$FWIDTH00 16718
$FHEIGHT00 16789
$VERSION 5.5
.DESCR
.INTERIOR
$DATE 29-01-2008
$TIME 08:53:36
$PARAFFINE00 1.40024800322806e-002 1.61919601816576e-005 -1.17365890292612e+002 1.52529249507402e-005 -1.40012354850240e-002 1.16656669342070e+002
$PARINVAFF00 7.14158304782664e+001 8.25900174795976e-002 8.37214784870798e+003 7.78002986768662e-002 -7.14221785163373e+001 8.34100456418987e+003
$FOC00 1.53250000000000e+002
.RELATIVE
.ABSOLUE
.RELEVEMENT
.EXTERNE
.CORRECTION
.CALIBRAGE
.TIN
.INDEX
.LISTMODELES
.CORRECTION
.EXTERIOR
$DATE 29-01-2008
$TIME 08:53:36
$XYZ00 1.94982930469953e+005 5.39416716624847e+006 2.75675570507451e+003
$OPK00 -8.62989093190236e-001 1.48065125998803e-001 -2.29624584291242e+000
.END
";
    const SAMPLE4: &str = r"
.GLOBAL
$DATE  2021-Aug-27
$TIME  14:49:48
$FSCALE00     4.9927968458E+004
$FILE00  Eastmain_202104008_IMG_30cm_412.TIF
$FWIDTH00       17310
$FHEIGHT00       11310
$TYPE OPK
.INTERIOR
$DATE  2021-Aug-27
$TIME  14:49:48
$PARAFFINE00         +6.00000E-003 +0.00000E+000 -5.19300E+001 +0.00000E+000 -6.00000E-003 +3.39300E+001
$PARINVAFF00         +1.66667E+002 +0.00000E+000 +8.65500E+003 +0.00000E+000 -1.66667E+002 +5.65500E+003
$FOC00       100.50000
.EXTERIOR
$XYZ00               +6.6955585388E+005 +5.7877236973E+006 +5.0677608300E+003
$OPK00               +7.9378000000E-001 -4.9160000000E-001 -8.9172580000E+001
$DATE  2021-Aug-27
$TIME  14:49:48
.CALIBRAGE
.CORRECTION
.END
";
    #[test]
    fn test_parse_par() {
        let par = PAR::try_from(SAMPLE).unwrap();
        println!("{par:#?}");
        assert_eq!(par.global.date, NaiveDate::from_ymd_opt(2025, 10, 23).unwrap());
        assert_eq!(par.global.time, NaiveTime::from_hms_opt(15, 43, 16).unwrap());
        assert_eq!(par.global.file, "Eastmain_202504033_IMG_15cm_105.tif");
        assert_eq!(par.global.image_width, 17310);
        assert_eq!(par.global.image_height, 11310);
        assert_eq!(par.global.ext_type, "OPK");
        assert_eq!(par.interior_orientation.date, NaiveDate::from_ymd_opt(2025, 10, 23).unwrap());
        assert_eq!(par.interior_orientation.time, NaiveTime::from_hms_opt(15, 43, 16).unwrap());
    }
    #[test]
    fn test_parse_par2() {
        let par = PAR::try_from(SAMPLE2).unwrap();
        println!("{par:#?}");
        assert_eq!(par.global.date, NaiveDate::from_ymd_opt(2025, 10, 14).unwrap());
        assert_eq!(par.global.time, NaiveTime::from_hms_opt(16, 13, 52).unwrap());
        assert_eq!(par.global.file, "Matimekosh_202504001_IMG_15cm_030.tif");
        assert_eq!(par.global.ext_type, "OPK");
        assert_eq!(par.interior_orientation.date, NaiveDate::from_ymd_opt(2025, 10, 14).unwrap());
        assert_eq!(par.interior_orientation.time, NaiveTime::from_hms_opt(16, 13, 52).unwrap());
        assert_eq!(par.camera_calibration.as_ref().unwrap().name, "EAGLE_ROTATION_0deg");
        assert_eq!(par.camera_calibration.as_ref().unwrap().focal_length, 79.8);
        assert_eq!(par.camera_calibration.as_ref().unwrap().principal_point, (11504.5, 7394.5));
        assert_eq!(par.camera_calibration.as_ref().unwrap().pixel_size, 4.6);
        assert_eq!(par.exterior_orientation.date, NaiveDate::from_ymd_opt(2025, 10, 14).unwrap());
        assert_eq!(par.exterior_orientation.time, NaiveTime::from_hms_opt(16, 13, 52).unwrap());
        assert_eq!(par.exterior_orientation.xyz.x, 350_412.453_33);
        assert_eq!(par.exterior_orientation.xyz.y, 6_075_079.381_89);
        assert_eq!(par.exterior_orientation.xyz.z, 3_132.036_98);
    }
    #[test]
    fn test_parse_par3() {
        let par = PAR::try_from(SAMPLE3).unwrap();
        println!("{par:#?}");
        assert_eq!(par.global.date, NaiveDate::from_ymd_opt(2008, 1, 29).unwrap());
        assert_eq!(par.global.time, NaiveTime::from_hms_opt(8, 53, 36).unwrap());
        assert_eq!(par.global.file, "A31926_112.TIF");
        assert_eq!(par.global.ext_type, "OPK");
        assert_eq!(par.interior_orientation.date, NaiveDate::from_ymd_opt(2008, 1, 29).unwrap());
        assert_eq!(par.interior_orientation.time, NaiveTime::from_hms_opt(8, 53, 36).unwrap());
        assert_eq!(par.exterior_orientation.date, NaiveDate::from_ymd_opt(2008, 1, 29).unwrap());
        assert_eq!(par.exterior_orientation.time, NaiveTime::from_hms_opt(8, 53, 36).unwrap());
        assert_eq!(par.exterior_orientation.xyz.x, 194_982.930_469_953);
        assert_eq!(par.exterior_orientation.xyz.y, 5_394_167.166_248_47);
        assert_eq!(par.exterior_orientation.xyz.z, 2_756.755_705_074_51);
    }
    #[test]
    fn test_parse_par4() {
        let par = PAR::try_from(SAMPLE4).unwrap();
        println!("{par:#?}");
        assert_eq!(par.global.date, NaiveDate::from_ymd_opt(2021, 8, 27).unwrap());
        assert_eq!(par.global.time, NaiveTime::from_hms_opt(14, 49, 48).unwrap());
        assert_eq!(par.global.file, "Eastmain_202104008_IMG_30cm_412.TIF");
        assert_eq!(par.global.ext_type, "OPK");
        assert_eq!(par.interior_orientation.date, NaiveDate::from_ymd_opt(2021, 8, 27).unwrap());
        assert_eq!(par.interior_orientation.time, NaiveTime::from_hms_opt(14, 49, 48).unwrap());
        assert_eq!(par.exterior_orientation.date, NaiveDate::from_ymd_opt(2021, 8, 27).unwrap());
        assert_eq!(par.exterior_orientation.time, NaiveTime::from_hms_opt(14, 49, 48).unwrap());
        assert_eq!(par.exterior_orientation.xyz.x, 669_555.853_88);
        assert_eq!(par.exterior_orientation.xyz.y, 5_787_723.697_3);
        assert_eq!(par.exterior_orientation.xyz.z, 5_067.760_83);
    }
}
