//! Read-only enumeration of ICC/ICM files in the Windows system color directory.
use crate::{Profile,R};
use std::{fs,path::{Path,PathBuf}};
#[derive(Debug,Clone)]
pub struct Entry{pub path:PathBuf,pub name:String,pub description:String,pub format:String,pub device_link:bool}
#[derive(Debug,Clone,Default)]
pub struct Catalog{pub entries:Vec<Entry>,pub excluded:usize,pub invalid:usize}
fn print_space(s:&[u8])->bool{
 s==b"CMYK"||(s.len()==4&&&s[1..]==b"CLR"&&matches!(s[0],b'2'..=b'9'|b'A'..=b'F'))
}
pub fn supported(p:&Profile)->bool{
 print_space(&p.header[16..20])&&(&p.header[12..16]==b"prtr"||(&p.header[12..16]==b"link"&&print_space(&p.header[20..24])))
}
pub fn directory()->R<PathBuf>{
 #[cfg(windows)]{
  // Native Windows color directory, including systems installed on a non-C drive.
  use std::os::windows::ffi::OsStringExt;
  #[link(name="mscms")]
  extern "system"{fn GetColorDirectoryW(machine:*const u16,buffer:*mut u16,size:*mut u32)->i32;}
  let mut buffer=vec![0u16;32768];let mut bytes=(buffer.len()*2) as u32;
  let ok=unsafe{GetColorDirectoryW(std::ptr::null(),buffer.as_mut_ptr(),&mut bytes)};
  if ok!=0{let n=buffer.iter().position(|&v|v==0).unwrap_or(buffer.len());if n>0{return Ok(PathBuf::from(std::ffi::OsString::from_wide(&buffer[..n])))}}
  let root=std::env::var_os("SystemRoot").ok_or("Windows color directory unavailable")?;
  Ok(PathBuf::from(root).join("System32").join("spool").join("drivers").join("color"))
 }
 #[cfg(not(windows))]{Err("System color directory picker is implemented for Windows".into())}
}
pub fn scan(root:&Path)->R<Catalog>{
 let mut out=Catalog::default();let mut count=0;
 for e in fs::read_dir(root).map_err(|e|e.to_string())?{
  let e=e.map_err(|e|e.to_string())?;let path=e.path();
  if !e.file_type().map_err(|e|e.to_string())?.is_file()||!path.extension().and_then(|x|x.to_str()).is_some_and(|x|x.eq_ignore_ascii_case("icc")||x.eq_ignore_ascii_case("icm")){continue}
  count+=1;if count>5000{return Err("System color folder exceeds 5000 ICC files".into())}
  let p=match Profile::read(&path){Ok(p)=>p,Err(_)=>{out.invalid+=1;continue}};
  if !supported(&p){out.excluded+=1;continue}
  if lcms2::Profile::new_icc(&p.bytes()?).is_err(){out.invalid+=1;continue}
  let device_link=&p.header[12..16]==b"link";
  let name=e.file_name().to_string_lossy().into_owned();
  let description=crate::tags::description(&p).unwrap_or_else(||name.clone());
  let format=format!("v{} · {} · {}",p.version_major(),p.space().trim(),if device_link{"DeviceLink"}else{"Output"});
  out.entries.push(Entry{path,name,description,format,device_link});
 }
 out.entries.sort_by(|a,b|a.name.to_lowercase().cmp(&b.name.to_lowercase()));Ok(out)
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn scans_supported_files_and_excludes_rgb(){
  let d=tempfile::tempdir().unwrap();let p=d.path().join("test.ICM");crate::synthetic_cmyk_v4(true).save_new(&p).unwrap();
  fs::write(d.path().join("rgb.icc"),lcms2::Profile::new_srgb().icc().unwrap()).unwrap();fs::write(d.path().join("bad.icc"),b"bad").unwrap();
  let c=scan(d.path()).unwrap();assert_eq!(c.entries.len(),1);assert_eq!(c.excluded,1);assert_eq!(c.invalid,1);assert!(!c.entries[0].device_link);
 }
}
