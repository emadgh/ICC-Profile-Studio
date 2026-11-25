//! Read-only enumeration of ICC/ICM files in the Windows system color directory.
use crate::{Profile,R,u32at};
use std::{fs,path::{Path,PathBuf}};
#[derive(Debug,Clone)]
pub struct Entry{pub path:PathBuf,pub name:String,pub description:String,pub format:String,pub device_link:bool}
#[derive(Debug,Clone,Default)]
pub struct Catalog{pub entries:Vec<Entry>,pub excluded:usize,pub invalid:usize}
fn sig(b:&[u8])->String{String::from_utf8_lossy(b).into_owned()}
fn text_preview(v:&[u8])->Option<String>{
 let text=if v.starts_with(b"mluc"){
  let count=u32at(v,8).ok()? as usize;let record=u32at(v,12).ok()? as usize;
  if count==0||record<12||16usize.checked_add(count.checked_mul(record)?)?>v.len(){return None}
  let mut out=Vec::new();
  for i in 0..count.min(4){
   let p=16+i*record;let n=u32at(v,p+4).ok()? as usize;let o=u32at(v,p+8).ok()? as usize;
   if n%2!=0{continue}let b=v.get(o..o.checked_add(n)?)?;
   let t=String::from_utf16_lossy(&b.chunks_exact(2).take(256).map(|w|u16::from_be_bytes([w[0],w[1]])).collect::<Vec<_>>());
   out.push(format!("{}: {}",sig(&v[p..p+4]),t));
  }
  out.join(" | ")
 }else if v.starts_with(b"desc"){
  let n=u32at(v,8).ok()? as usize;let b=v.get(12..12usize.checked_add(n)?)?;sig(&b[..b.iter().position(|&x|x==0).unwrap_or(b.len()).min(256)])
 }else if v.starts_with(b"text"){
  let b=v.get(8..)?;sig(&b[..b.iter().position(|&x|x==0).unwrap_or(b.len()).min(256)])
 }else{return None};
 Some(text.chars().map(|c|if c.is_control(){' '}else{c}).take(1024).collect())
}
fn description(p:&Profile)->Option<String>{p.tags.get(b"desc").and_then(|v|text_preview(v)).filter(|s|!s.is_empty())}

fn print_space(s:&[u8])->bool{
 crate::print_space::format(s).is_ok()
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
  let description=description(&p).unwrap_or_else(||name.clone());
  let format=format!("v{} · {} · {}",p.version_major(),p.space().trim(),if device_link{"DeviceLink"}else{"Output"});
  out.entries.push(Entry{path,name,description,format,device_link});
 }
 out.entries.sort_by(|a,b|a.name.to_lowercase().cmp(&b.name.to_lowercase()));Ok(out)
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn native_cmy_is_listed(){
  let d=tempfile::tempdir().unwrap();crate::print_space::fixture_cmy().save_new(&d.path().join("cmy.icc")).unwrap();
  let c=scan(d.path()).unwrap();assert_eq!(c.entries.len(),1);assert!(c.entries[0].format.contains("CMY"));assert!(!c.entries[0].device_link);
 }
 #[test]fn scans_supported_files_and_excludes_rgb(){
  let d=tempfile::tempdir().unwrap();let p=d.path().join("test.ICM");crate::synthetic_cmyk_v4(true).save_new(&p).unwrap();
  fs::write(d.path().join("rgb.icc"),lcms2::Profile::new_srgb().icc().unwrap()).unwrap();fs::write(d.path().join("bad.icc"),b"bad").unwrap();
  let c=scan(d.path()).unwrap();assert_eq!(c.entries.len(),1);assert_eq!(c.excluded,1);assert_eq!(c.invalid,1);assert!(!c.entries[0].device_link);
 }
}
