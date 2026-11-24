//! Generic ICC tag directory, bounded decoding and conservative metadata edits.
use crate::{Profile,R,u16at,u32at,put16,put32};
use std::{path::Path,io::Write};
#[derive(Clone,Debug)]
pub struct Entry{pub signature:[u8;4],pub kind:String,pub size:usize,pub summary:String,pub hex:String}
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
pub fn description(p:&Profile)->Option<String>{p.tags.get(b"desc").and_then(|v|text_preview(v)).filter(|s|!s.is_empty())}
fn detail(v:&[u8])->String{
 if let Some(t)=text_preview(v){return t}
 if v.starts_with(b"XYZ ")&&v.len()>=20{
  let f=|i|u32at(v,i).unwrap_or(0) as i32 as f64/65536.;
  return format!("XYZ {:.6}, {:.6}, {:.6}",f(8),f(12),f(16))
 }
 if v.starts_with(b"curv"){return match u32at(v,8){Ok(0)=>"Identity curve".into(),Ok(1)=>u16at(v,12).map(|x|format!("Gamma {:.4}",x as f64/256.)).unwrap_or_else(|_|"Truncated gamma".into()),Ok(n)=>format!("{n} sampled curve entries"),Err(e)=>e}}
 if v.starts_with(b"para"){return u16at(v,8).map(|n|format!("Parametric curve function {n} (read-only)")).unwrap_or_else(|e|e)}
 if v.len()>=12&&(v.starts_with(b"mft1")||v.starts_with(b"mft2")){return format!("{} → {} channels · grid {}",v[8],v[9],v[10])}
 if v.len()>=32&&(v.starts_with(b"mAB ")||v.starts_with(b"mBA ")){
  let off=u32at(v,24).unwrap_or(0) as usize;
  let grid=if off>=32&&v[8]>0&&v[8]<=16{v.get(off..off.saturating_add(v[8] as usize)).map(|x|format!("{x:?}")).unwrap_or("invalid".into())}else{"not present".into()};
  return format!("{} → {} channels · CLUT axes {grid}",v[8],v[9])
 }
 if v.starts_with(b"mpet")&&v.len()>=16{return format!("{} → {} channels · {} process elements (read-only)",u16at(v,8).unwrap_or(0),u16at(v,10).unwrap_or(0),u32at(v,12).unwrap_or(0))}
 "Raw payload (not interpreted)".into()
}
pub fn list(p:&Profile)->Vec<Entry>{
 p.tags.iter().map(|(k,v)|Entry{signature:*k,kind:sig(&v[..4]),size:v.len(),summary:detail(v),hex:v.iter().take(64).map(|b|format!("{b:02X}")).collect::<Vec<_>>().join(" ")}).collect()
}
pub fn export(p:&Profile,key:[u8;4],path:&Path)->R<()>{
 let raw=p.tags.get(&key).ok_or("Tag not found")?;
 let parent=path.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(Path::new("."));
 let mut f=tempfile::NamedTempFile::new_in(parent).map_err(|e|e.to_string())?;
 f.write_all(raw).map_err(|e|e.to_string())?;f.as_file().sync_all().map_err(|e|e.to_string())?;f.persist_noclobber(path).map_err(|e|e.to_string())?;Ok(())
}
pub const TEXT_TAGS:[[u8;4];4]=[*b"desc",*b"cprt",*b"dmnd",*b"dmdd"];
pub fn edit_text(p:&Profile,key:[u8;4],text:&str)->R<Profile>{
 if !TEXT_TAGS.contains(&key){return Err("Only standard text metadata is editable".into())}
 if text.trim().is_empty()||text.len()>4096||text.contains('\0')||text.chars().any(|c|c.is_control()&&!matches!(c,'\r'|'\n'|'\t')){return Err("Use nonempty text up to 4096 UTF-8 bytes, without embedded NUL".into())}
 let mut t=Vec::new();
 match p.version_major(){
  4=>{let chars:Vec<u16>=text.encode_utf16().collect();t.extend(b"mluc\0\0\0\0");put32(&mut t,1);put32(&mut t,12);t.extend(b"enUS");put32(&mut t,(chars.len()*2) as u32);put32(&mut t,28);for c in chars{put16(&mut t,c);}},
  2=>{if !text.is_ascii(){return Err("This v2 metadata editor requires ASCII; Unicode is supported in v4".into())}
   if key==*b"cprt"{t.extend(b"text\0\0\0\0");t.extend(text.as_bytes());t.push(0);}
   else{t.extend(b"desc\0\0\0\0");put32(&mut t,(text.len()+1) as u32);t.extend(text.as_bytes());t.push(0);put32(&mut t,0);put32(&mut t,0);put16(&mut t,0);t.push(0);t.extend([0;67]);}
  },
  _=>return Err("Only ICC v2/v4 text metadata is supported".into())
 }
 let mut out=p.clone();out.tags.insert(key,t);
 let bytes=out.bytes()?;lcms2::Profile::new_icc(&bytes).map_err(|e|format!("LittleCMS rejected metadata copy: {e}"))?;
 Ok(out)
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn metadata_preserves_all_other_payloads(){
  let mut p=crate::synthetic_cmyk_v4(true);p.tags.insert(*b"priv",b"data\0\0\0\0test".to_vec());
  let o=edit_text(&p,*b"desc","New profile \u{00e9}").unwrap();assert!(description(&o).unwrap().contains("New profile"));
  for(k,v)in &p.tags{if k!=b"desc"{assert_eq!(o.tags.get(k),Some(v));}}
  assert!(edit_text(&p,*b"A2B1","no").is_err());
  assert!(list(&o).iter().any(|t|t.signature==*b"priv"&&t.kind=="data"));
  let d=tempfile::tempdir().unwrap();let f=d.path().join("raw.bin");export(&o,*b"priv",&f).unwrap();assert_eq!(std::fs::read(&f).unwrap(),p.tags[b"priv"]);assert!(export(&o,*b"priv",&f).is_err());
 }
}
