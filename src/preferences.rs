//! Explicit local preferences; never persist device-specific ink limits or
//! silently resume a production job. Workspace settings remain opt-in.
use serde::{Serialize,Deserialize};
use std::{path::{Path,PathBuf},fs,io::Write};
type R<T>=Result<T,String>;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(default,deny_unknown_fields)]
pub struct Preferences {
 pub schema:u32,pub preview_open:bool,pub three_d:bool,pub wireframe:bool,pub solid:bool,pub profile_colors:bool,pub convex:bool,pub direct_image:bool,
 pub gamut_grid:usize,pub compare_grid:usize,pub dpi:u32,pub stream_enabled:bool,
 pub bigtiff:bool,pub separations:bool,pub proof_media:bool,pub proof_bpc:bool,
 pub monitor:Option<PathBuf>,pub library:Option<PathBuf>,pub output:Option<PathBuf>,
}
impl Default for Preferences{
 fn default()->Self{Self{schema:1,preview_open:true,three_d:true,wireframe:false,solid:true,profile_colors:true,convex:false,direct_image:true,gamut_grid:17,compare_grid:17,dpi:300,stream_enabled:false,bigtiff:false,separations:false,proof_media:true,proof_bpc:false,monitor:None,library:None,output:None}}
}
impl Preferences{
 pub fn validate(&self)->R<()>{
  if self.schema!=1||![9,17,33].contains(&self.gamut_grid)||![9,17,33].contains(&self.compare_grid)||!(50..=2400).contains(&self.dpi){return Err("Unsupported preferences schema or setting range".into())}
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
 p.validate()?;Ok(p)
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
 #[test]fn roundtrip_and_update(){let d=tempfile::tempdir().unwrap();let p=d.path().join("settings.json");assert_eq!(load(&p).unwrap(),Preferences::default());let mut v=Preferences::default();v.dpi=600;v.preview_open=false;save(&p,&v).unwrap();assert_eq!(load(&p).unwrap(),v);v.dpi=360;save(&p,&v).unwrap();assert_eq!(load(&p).unwrap(),v);}
 #[test]fn rejects_invalid(){let d=tempfile::tempdir().unwrap();let p=d.path().join("settings.json");fs::write(&p,b"{\"schema\":1,\"dpi\":0}").unwrap();assert!(load(&p).is_err());fs::write(&p,b"{\"schema\":99}").unwrap();assert!(load(&p).is_err());}
}
