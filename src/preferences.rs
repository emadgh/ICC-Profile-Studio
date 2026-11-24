//! Explicit local preferences; never persist device-specific ink limits or
//! silently resume a production job. Workspace settings remain opt-in.
use serde::{Serialize,Deserialize};
use std::{path::{Path,PathBuf},fs,io::Write};
type R<T>=Result<T,String>;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(default)] // Ignore retired library and image-workflow keys in older preferences.
pub struct Preferences {
 pub schema:u32,pub preview_open:bool,pub three_d:bool,pub wireframe:bool,pub solid:bool,pub profile_colors:bool,pub language:crate::i18n::Language,pub preview_width:f32,pub opacity_2d:u8,pub layout:crate::gamut_raster::CompareLayout,pub floor_contour:bool,
 pub gamut_grid:usize,pub compare_grid:usize,pub chart_dpi:u32,
}
impl Default for Preferences{
 fn default()->Self{Self{schema:1,preview_open:true,three_d:true,wireframe:false,solid:true,profile_colors:true,language:crate::i18n::Language::En,preview_width:360.,opacity_2d:100,layout:crate::gamut_raster::CompareLayout::default(),floor_contour:true,gamut_grid:33,compare_grid:17,chart_dpi:300}}
}
impl Preferences{
 pub fn validate(&self)->R<()>{
  if self.schema!=1||!self.preview_width.is_finite()||!(280. ..=4000.).contains(&self.preview_width)||self.opacity_2d>100||![9,17,33].contains(&self.gamut_grid)||![9,17,33].contains(&self.compare_grid)||!(50..=1200).contains(&self.chart_dpi){return Err("Unsupported preferences schema or setting range".into())}
  Ok(())
 }
}
pub fn location()->R<PathBuf>{
 let root=std::env::var_os("APPDATA").filter(|v|!v.is_empty()).ok_or("APPDATA unavailable; preferences cannot be stored")?;
 Ok(PathBuf::from(root).join("ICC Profile Studio").join("preferences.json"))
}
pub fn load(path:&Path)->R<Preferences>{
 if !path.exists(){return Ok(Preferences::default())}
 if fs::metadata(path).map_err(|e|e.to_string())?.len()>128*1024{return Err("Preferences exceed 128 KiB".into())}
 let p:Preferences=serde_json::from_slice(&fs::read(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 p.validate()?;Ok(Preferences{gamut_grid:33,..p})
}
pub fn save(path:&Path,p:&Preferences)->R<()>{
 p.validate()?;let parent=path.parent().ok_or("No preferences folder")?;fs::create_dir_all(parent).map_err(|e|e.to_string())?;
 let mut file=tempfile::NamedTempFile::new_in(parent).map_err(|e|e.to_string())?;
 let bytes=serde_json::to_vec_pretty(p).map_err(|e|e.to_string())?;
 file.write_all(&bytes).map_err(|e|e.to_string())?;file.as_file().sync_all().map_err(|e|e.to_string())?;
 file.persist(path).map_err(|e|e.to_string())?;Ok(())
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn roundtrip_and_update(){let d=tempfile::tempdir().unwrap();let p=d.path().join("settings.json");assert_eq!(load(&p).unwrap(),Preferences::default());let mut v=Preferences::default();v.chart_dpi=600;v.preview_open=false;v.language=crate::i18n::Language::Fa;save(&p,&v).unwrap();assert_eq!(load(&p).unwrap(),v);v.chart_dpi=360;save(&p,&v).unwrap();assert_eq!(load(&p).unwrap(),v);}
 #[test]fn retired_image_settings_are_ignored(){
 let d=tempfile::tempdir().unwrap();let p=d.path().join("settings.json");
 fs::write(&p,br#"{"schema":1,"preview_open":false,"dpi":2400,"stream_enabled":true,"bigtiff":true,"proof_media":false,"monitor":"old.icc","output":"old","library":"old"}"#).unwrap();
 let v=load(&p).unwrap();assert!(!v.preview_open);assert_eq!(v.chart_dpi,300);
 save(&p,&v).unwrap();let json:serde_json::Value=serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();assert!(json.get("stream_enabled").is_none());assert!(json.get("monitor").is_none());assert!(json.get("dpi").is_none());assert!(json.get("library").is_none());
 }
 #[test]fn rejects_invalid(){let d=tempfile::tempdir().unwrap();let p=d.path().join("settings.json");fs::write(&p,b"{\"schema\":1,\"chart_dpi\":0}").unwrap();assert!(load(&p).is_err());fs::write(&p,b"{\"schema\":99}").unwrap();assert!(load(&p).is_err());}
}
