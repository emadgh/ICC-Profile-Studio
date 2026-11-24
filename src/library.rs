//! Local profile catalog. Does not alter or upload ICC files. A CSV manifest
//! carries user-supplied production metadata, independently of ICC internals.
use std::{collections::BTreeMap,fs,path::{Path,PathBuf}};
use serde::{Deserialize,Serialize};
type R<T>=Result<T,String>;
#[derive(Debug,Clone,Default,Serialize,Deserialize)]
pub struct Metadata{pub filename:String,pub line:String,pub printer:String,pub glaze:String,pub ink_batch:String,pub firing:String,pub notes:String}
#[derive(Debug,Clone)]
pub struct Entry{pub path:PathBuf,pub meta:Metadata,pub format:String,pub valid:bool}
fn err(e:impl std::fmt::Display)->String{e.to_string()}
fn metadata(root:&Path)->R<BTreeMap<String,Metadata>>{
 let path=root.join("profiles.csv");
 if !path.exists(){return Ok(BTreeMap::new())}
 let mut r=csv::Reader::from_path(path).map_err(err)?;let mut out=BTreeMap::new();
 for row in r.deserialize::<Metadata>(){let m=row.map_err(err)?;if m.filename.is_empty()||out.insert(m.filename.clone(),m).is_some(){return Err("Empty or duplicate profile filename in manifest".into())}}
 Ok(out)
}
pub fn scan(root:&Path)->R<Vec<Entry>>{
 let known=metadata(root)?;let mut out=Vec::new();
 for e in fs::read_dir(root).map_err(err)?{
  let e=e.map_err(err)?;if !e.file_type().map_err(err)?.is_file(){continue}
  let p=e.path();if !p.extension().and_then(|v|v.to_str()).is_some_and(|s|s.eq_ignore_ascii_case("icc")||s.eq_ignore_ascii_case("icm")){continue}
  let name=e.file_name().to_string_lossy().into_owned();
  let m=known.get(&name).cloned().unwrap_or_else(||Metadata{filename:name,..Default::default()});
  let parsed=crate::Profile::read(&p).and_then(|v|{let _=lcms2::Profile::new_icc(&v.bytes()?).map_err(err)?;Ok(v)});
  let (format,valid)=match parsed{Ok(v)=>(format!("ICC v{} / {} / {}",v.version_major(),v.space(),v.pcs()),true),Err(e)=>(e,false)};
  out.push(Entry{path:p,meta:m,format,valid});
  if out.len()>5000{return Err("Catalog supports up to 5000 profiles per folder".into())}
 }
 out.sort_by(|a,b|a.meta.filename.cmp(&b.meta.filename));Ok(out)
}
pub fn write_manifest(root:&Path,entries:&[Entry])->R<PathBuf>{
 let path=root.join("profiles.csv");
 let mut f=tempfile::NamedTempFile::new_in(root).map_err(err)?;
 {let mut w=csv::WriterBuilder::new().has_headers(false).from_writer(f.as_file_mut());
 w.write_record(["filename","line","printer","glaze","ink_batch","firing","notes"]).map_err(err)?;
 for e in entries{w.serialize(&e.meta).map_err(err)?;}w.flush().map_err(err)?;}
 f.persist_noclobber(&path).map_err(err)?;Ok(path)
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn manifest_no_overwrite(){let d=tempfile::tempdir().unwrap();write_manifest(d.path(),&[]).unwrap();assert!(write_manifest(d.path(),&[]).is_err());assert!(scan(d.path()).unwrap().is_empty());}
}
