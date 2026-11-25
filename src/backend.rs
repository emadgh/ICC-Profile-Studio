//! Shared profile generation service, independent of UI frameworks.
use crate::Profile;
use std::{fs,path::{Path,PathBuf},process::Command};
#[cfg(windows)] use std::os::windows::process::CommandExt;
type R<T>=Result<T,String>;
const URL:&str="https://www.argyllcms.com/Argyll_V3.5.0_win64_exe.zip";
pub(crate) fn hidden(c:&mut Command)->&mut Command{
 #[cfg(windows)]{c.creation_flags(0x08000000);}
 c
}
fn find_exe(p:&Path,depth:u8)->Option<PathBuf>{
 if depth==0{return None}
 for e in fs::read_dir(p).ok()?.flatten(){let path=e.path();let ty=e.file_type().ok()?;if ty.is_file()&&e.file_name().to_string_lossy().eq_ignore_ascii_case("revfix.exe"){return Some(path)}if ty.is_dir(){if let Some(x)=find_exe(&path,depth-1){return Some(x)}}}None
}
pub(crate) fn engine(progress:&impl Fn(String))->R<PathBuf>{
 if !cfg!(all(windows,target_arch="x86_64")){return Err("Automatic Argyll installation requires Windows x64".into())}
 let exe=std::env::current_exe().map_err(|e|e.to_string())?;let parent=exe.parent().ok_or("Executable directory unavailable")?;
 let dest=parent.join("argyll-3.5.0");
 if let Some(p)=find_exe(&dest,5){return Ok(p)}
 if dest.exists(){return Err("Incomplete argyll-3.5.0 folder; rename or remove it, then retry".into())}
 progress("Downloading official ArgyllCMS 3.5.0 for Windows x64...".into());
 let temp=tempfile::tempdir_in(parent).map_err(|e|format!("Portable directory must be writable: {e}"))?;
 let zip=temp.path().join("argyll.zip");let stage=temp.path().join("unpacked");
 let ps=PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(||"C:\\Windows".into())).join("System32/WindowsPowerShell/v1.0/powershell.exe");
 let script=r#"$ErrorActionPreference='Stop'; [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -UseBasicParsing -Uri $env:ICC_ARGYLL_URL -OutFile $env:ICC_ARGYLL_ZIP; if ($env:ARGYLL_SHA256) { if ((Get-FileHash -Algorithm SHA256 -LiteralPath $env:ICC_ARGYLL_ZIP).Hash -ne $env:ARGYLL_SHA256) { throw 'Argyll archive SHA256 mismatch' } }; Expand-Archive -LiteralPath $env:ICC_ARGYLL_ZIP -DestinationPath $env:ICC_ARGYLL_STAGE"#;
 let result=hidden(&mut Command::new(ps)).args(["-NoProfile","-NonInteractive","-Command",script]).env("ICC_ARGYLL_URL",URL).env("ICC_ARGYLL_ZIP",&zip).env("ICC_ARGYLL_STAGE",&stage).output().map_err(|e|e.to_string())?;
 if !result.status.success(){return Err(format!("Dependency download failed: {}",String::from_utf8_lossy(&result.stderr)))}
 if find_exe(&stage,5).is_none(){return Err("Official archive did not contain revfix.exe".into())}
 fs::rename(&stage,&dest).map_err(|e|e.to_string())?;
 find_exe(&dest,5).ok_or("Missing revfix.exe after extraction".into())
}
#[derive(Clone,Copy,PartialEq,Eq)]
pub enum Mode{Argyll,Custom}
pub struct Job{pub source:PathBuf,pub destination:PathBuf,pub mode:Mode,pub caps:[f64;4],pub tac:f64,pub estimate:bool,pub grid:usize}
pub fn run(j:Job,progress:impl Fn(String))->R<String>{
 let reference=Profile::read(&j.source)?;reference.check()?;
 if j.destination.exists(){return Err("Output already exists. Choose a new filename; the reference is never overwritten.".into())}
 let caps=j.caps.map(|v|v/100.);
 if j.mode==Mode::Argyll && (reference.version_major()!=2||!reference.cmyk()||caps[..3].iter().any(|v|*v!=1.)){return Err("Argyll revfix requires ICC v2 CMYK and K-only limits. Use Custom for ICC v4 or other channels.".into())}
 let mut output=match j.mode{
  Mode::Custom=>crate::multichannel::rebuild(&reference,&caps,j.tac/100.,j.grid,&progress)?,
  Mode::Argyll=>{
   let exe=engine(&progress)?;
   let dir=tempfile::tempdir().map_err(|e|e.to_string())?;
   let input=dir.path().join("input.icc");let dest=dir.path().join("output.icc");
   fs::copy(&j.source,&input).map_err(|e|e.to_string())?;
   progress("Argyll revfix is rebuilding all three B2A tables...".into());
   let mut cmd=Command::new(&exe);
   hidden(&mut cmd).current_dir(exe.parent().unwrap()).args(["-v","-0","-1","-2",if caps[3]==0.{"-kz"}else{"-kr"}]).arg(format!("-L{}",j.caps[3])).arg(format!("-r{}",j.grid));
   if !j.estimate{cmd.arg(format!("-l{}",j.tac));}
   let result=cmd.arg(&input).arg(&dest).output().map_err(|e|e.to_string())?;
   if !result.status.success(){return Err(format!("revfix failed ({:?}):\n{}\n{}",result.status.code(),String::from_utf8_lossy(&result.stderr),String::from_utf8_lossy(&result.stdout)))}
   Profile::read(&dest)?
  }
 };
 if output.space()!=reference.space() || output.pcs()!=reference.pcs(){return Err("Engine changed the color space or PCS".into())}
 progress("Validating maximum channel values in all output LUTs...".into());
 let (bounds,native)=crate::validation::verify_output(&output,caps,if j.estimate&&j.mode==Mode::Argyll{None}else{Some(j.tac/100.)}).map_err(|e|format!("{e}\nIf Argyll cannot meet the strict bound, try Custom with an explicit TAC."))?;
 let max=bounds.channels;
 output.clean_stale();
 let name=j.destination.file_stem().unwrap_or_default().to_string_lossy();
 output.describe(&name);
 output.save_new(&j.destination)?;
 Ok(format!("Saved: {}\nVerified maxima (device values): {:.3}% / {:.3}% / {:.3}% / {:.3}%\nTAC upper bound: {:.3}% | LittleCMS: {} pipeline samples, peak {:.3}%\nTest a printed chart before production.",j.destination.display(),max[0]*100.,max[1]*100.,max[2]*100.,max[3]*100.,bounds.tac*100.,native.count,native.tac*100.))
}

pub fn run_multichannel(source:&Path,destination:&Path,caps:&[f64],tac:f64,grid:usize,progress:impl Fn(String))->R<String>{
 run_multichannel_controlled(source,destination,caps,tac,grid,&crate::multichannel::RebuildControl::default(),progress)
}
pub fn run_multichannel_controlled(source:&Path,destination:&Path,caps:&[f64],tac:f64,grid:usize,control:&crate::multichannel::RebuildControl,progress:impl Fn(String))->R<String>{
 control.check()?;let reference=Profile::read(source)?;
 if destination.exists(){return Err("Choose a new output filename; reference is never overwritten".into())}
 let limits:Vec<_>=caps.iter().map(|v|v/100.).collect();
 let (mut output,result)=crate::multichannel::rebuild_verified(&reference,&limits,tac/100.,grid,control,&progress)?;
 output.describe(&destination.file_stem().unwrap_or_default().to_string_lossy());
 control.begin_publish()?;output.save_new(destination)?;
 Ok(format!("Saved: {}\nVerified channel ceilings: {}%\nTAC bound {:.3}% · {} native samples · sampled TAC {:.3}%. All intents rebuilt from relative A2B1; validate actual output.",destination.display(),result.channels.iter().map(|v|format!("{:.3}",100.*v)).collect::<Vec<_>>().join(" / "),100.*result.tac,result.samples,100.*result.sampled_tac))
}

#[cfg(test)]mod controlled_tests{
 use super::*;
 #[test]fn cancellation_does_not_publish(){
  let d=tempfile::tempdir().unwrap();let source=d.path().join("source.icc");let dest=d.path().join("out.icc");
  crate::print_space::fixture(6).save_new(&source).unwrap();let before=fs::read(&source).unwrap();
  let c=crate::multichannel::RebuildControl::default();
  let e=run_multichannel_controlled(&source,&dest,&[100.;6],200.,9,&c,|_|{c.cancel();}).unwrap_err();
  assert_eq!(e,"Rebuild cancelled");assert!(!dest.exists());assert_eq!(fs::read(&source).unwrap(),before);
 }
}
