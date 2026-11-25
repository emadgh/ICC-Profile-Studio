//! Structural conversion between simple 16-bit v2 and v4 CMYK/nCLR Lab LUTs.
//! ICC legacy lut16 PCS Lab encoding has nominal full scale 65280,
//! whereas mAB/mBA use 65535. Adapt PCS-side curves, never device values.
//! https://www.color.org/v2issues/
use super::{Profile,Lut,R,interp,u32at,put32,put16};
use lcms2::{Profile as Native,Transform,Intent,Flags,PixelFormat,CIELab,CIELabExt,CIEXYZ,CIEXYZExt};
use std::path::Path;
const LEGACY:f64=65280./65535.;
const TRANSFORMS:[[u8;4];6]=[*b"A2B0",*b"A2B1",*b"A2B2",*b"B2A0",*b"B2A1",*b"B2A2"];
#[derive(Debug,Clone)]
pub struct IntentCheck{pub name:&'static str,pub max_de:f64,pub mean_de:f64,pub max_ink_pp:f64,pub max_tac_pp:f64,pub passed:bool,pub required:bool}
#[derive(Debug,Clone)]
pub struct Report{pub target:u8,pub relative_absolute_only:bool,pub checks:Vec<IntentCheck>,pub omitted:Vec<String>,pub forward_samples:usize,pub reverse_samples:usize,pub bounds:crate::print_space::Bounds,pub channels:Vec<String>,pub colorant_metadata:Vec<String>}
fn is_text(k:&[u8;4])->bool{matches!(k,b"desc"|b"cprt"|b"dmnd"|b"dmdd")}
fn shared(k:&[u8;4],v:&[u8])->bool{
 let expected=match k{b"wtpt"|b"bkpt"|b"lumi"=>b"XYZ ",b"chad"=>b"sf32",b"view"=>b"view",b"tech"=>b"sig ",b"meas"=>b"meas",_=>return false};
 v.starts_with(expected)
}
// Preserve a well-formed channel table/order as opaque metadata, without
// rescaling its coordinates or using it to infer physical ink identity.
// v2 files carrying these later tags are compatibility copies, not a
// claim of strict ICC v2 conformance.
fn ink_metadata(p:&Profile,k:&[u8;4],v:&[u8])->bool{
 let n=p.standard_channels().len();
 if !(2..=15).contains(&n){return false}
 if k==b"clrt"{
  if !v.starts_with(b"clrt")||u32at(v,8).ok()!=Some(n as u32)||v.len()!=12+38*n{return false}
  let names=p.colorants();
  if names.len()!=n||names.iter().any(Option::is_none){return false}
  let mut unique=std::collections::HashSet::new();
  for i in 0..n{
   let name=&v[12+38*i..44+38*i];
   let Some(end)=name.iter().position(|&x|x==0)else{return false};
   if end==0||!name[..end].iter().all(|x|x.is_ascii()&&!x.is_ascii_control())||!unique.insert(&name[..end]){return false}
  }
  return true
 }
 if k==b"clro"{
  if !v.starts_with(b"clro")||u32at(v,8).ok()!=Some(n as u32)||v.len()!=12+n{return false}
  let mut seen=[false;15];for &j in &v[12..]{let j=j as usize;if j>=n||seen[j]{return false}seen[j]=true;}return true
 }
 false
}
fn alternate(k:&[u8;4])->bool{k.starts_with(b"D2B")||k.starts_with(b"B2D")}
pub fn omitted_tags(p:&Profile)->Vec<String>{
 p.tags.iter().filter(|(k,v)|!TRANSFORMS.contains(k)&&!is_text(k)&&!shared(k,v)&&!ink_metadata(p,k,v)).map(|(k,_)|String::from_utf8_lossy(k).into_owned()).collect()
}
fn ascii_text(v:&[u8])->R<String>{
 let raw=if v.starts_with(b"mluc"){
  if u32at(v,8)?!=1||u32at(v,12)?!=12||v.get(16..20)!=Some(b"enUS".as_slice()){return Err("Only single en-US text records can be converted without losing localization".into())}
  let n=u32at(v,20)? as usize;let o=u32at(v,24)? as usize;
  let data=v.get(o..o.checked_add(n).ok_or("Text overflow")?).ok_or("Invalid mluc text")?;
  if n%2!=0{return Err("Invalid UTF-16 length".into())}
  String::from_utf16(&data.chunks_exact(2).map(|x|u16::from_be_bytes([x[0],x[1]])).collect::<Vec<_>>()).map_err(|e|e.to_string())?
 }else if v.starts_with(b"desc"){
  let n=u32at(v,8)? as usize;if n==0{return Err("Empty legacy description".into())}
  let end=12usize.checked_add(n).ok_or("Text overflow")?;
  let d=v.get(12..end).ok_or("Truncated desc")?;
  if d.last()!=Some(&0){return Err("Missing desc terminator".into())}
  let ascii=std::str::from_utf8(&d[..n-1]).map_err(|e|e.to_string())?.to_owned();
  let unicode_count=u32at(v,end+4)? as usize;
  let uend=(end+8).checked_add(unicode_count.checked_mul(2).ok_or("Text overflow")?).ok_or("Text overflow")?;
  let u=v.get(end+8..uend).ok_or("Truncated desc Unicode")?;
  if unicode_count>0{
   let unicode=String::from_utf16(&u.chunks_exact(2).map(|x|u16::from_be_bytes([x[0],x[1]])).collect::<Vec<_>>()).map_err(|e|e.to_string())?;
   if unicode.trim_end_matches('\0')!=ascii{return Err("Different localized desc content cannot be converted safely".into())}
  }
  if v.get(uend+2).copied().unwrap_or(1)!=0{return Err("Legacy ScriptCode text cannot be converted safely".into())}
  ascii
 }else if v.starts_with(b"text"){
  let d=v.get(8..).ok_or("Invalid text")?;let end=d.iter().position(|&b|b==0).ok_or("Missing text terminator")?;
  if d[end..].iter().any(|&b|b!=0){return Err("Unexpected trailing text data".into())}
  std::str::from_utf8(&d[..end]).map_err(|e|e.to_string())?.to_owned()
 }else{return Err("Unsupported text type".into())};
 if raw.is_empty()||!raw.is_ascii()||raw.chars().any(|c|c=='\0'||(c.is_control()&&!matches!(c,'\n'|'\r'|'\t'))){return Err("Only nonempty ASCII metadata is supported for lossless version conversion".into())}
 Ok(raw)
}
fn text_tag(s:&str,target:u8,copyright:bool)->Vec<u8>{
 let mut t=Vec::new();
 if target==4{
  t.extend(b"mluc\0\0\0\0");put32(&mut t,1);put32(&mut t,12);t.extend(b"enUS");put32(&mut t,(2*s.len()) as u32);put32(&mut t,28);
  for c in s.encode_utf16(){put16(&mut t,c);}
 }else if copyright{
  t.extend(b"text\0\0\0\0");t.extend(s.as_bytes());t.push(0);
 }else{
  t.extend(b"desc\0\0\0\0");put32(&mut t,(s.len()+1) as u32);t.extend(s.as_bytes());t.push(0);put32(&mut t,0);put32(&mut t,0);put16(&mut t,0);t.push(0);t.extend([0;67]);
 }
 t
}
fn common_lengths(curves:&mut [Vec<f64>])->R<()>{
 // v2 requires a common table length. Use the least common multiple of
 // segment counts so every original knot is retained; only final 16-bit
 // quantization remains. Refuse if this exceeds the format limit.
 fn gcd(mut a:usize,mut b:usize)->usize{while b!=0{let r=a%b;a=b;b=r;}a}
 let mut segments=1usize;
 for c in curves.iter(){
  let n=c.len().checked_sub(1).filter(|&n|n>0).ok_or("Invalid curve")?;
  segments=(segments/gcd(segments,n)).checked_mul(n).filter(|&x|x<=65534).ok_or("Cannot preserve all curve knots in a v2 common table")?;
 }
 for c in curves.iter_mut(){if c.len()!=segments+1{*c=(0..=segments).map(|i|interp(c,i as f64/segments as f64)).collect();}}
 Ok(())
}
fn build(p:&Profile,target:u8,allow_omit:bool)->R<(Profile,Vec<String>)>{
 let n=crate::editing::editable_channel_count(p)?;
 if p.pcs()!="Lab "||!matches!((p.version_major(),target),(2,4)|(4,2)){return Err("Requires opposite version of a CMYK/nCLR Lab printer ICC (v2 or v4)".into())}
 if p.tags.keys().any(alternate){return Err("Alternate D2B/B2D transforms cannot be converted".into())}
 let omitted=omitted_tags(p);
 if !omitted.is_empty()&&!allow_omit{return Err(format!("Unsupported tags require explicit omission consent: {}",omitted.join(", ")))}
 let mut out=p.clone();
 out.header[8..12].copy_from_slice(if target==4{&[4,0x30,0,0]}else{&[2,0x40,0,0]});
 out.header[84..128].fill(0); // v2 reserved / v4 profile ID and reserved
 out.tags.clear();
 for (k,v) in &p.tags{
  if TRANSFORMS.contains(k){
   let forward=k[0]==b'A';
   let expected=if p.version_major()==2{b"mft2"}else if forward{b"mAB "}else{b"mBA "};
   if !v.starts_with(expected){return Err(format!("Unsupported LUT type in {}",String::from_utf8_lossy(k)))}
   let mut l=Lut::parse(v)?;
   if (l.inputs,l.outputs)!=if forward{(n,3)}else{(3,n)}{return Err("Unexpected transform dimensions".into())}
   if forward{
    // Scale only the PCS output curve samples, preserving knots and CLUT.
    let f=if target==2{LEGACY}else{1./LEGACY};
    for curve in &mut l.out_curves{
     // Legacy identity output curves often reach 65535 at unused input
     // values even when the CLUT only reaches 65280. Do not reject those
     // valid profiles. Sample the composed clipped curve if its upper
     // range crosses the nominal Lab limit; never just scale endpoints.
     if target==4&&curve.iter().any(|&x|x>LEGACY){
      let n=curve.len().max(16385);
      *curve=(0..n).map(|i|(interp(curve,i as f64/(n-1) as f64)*f).clamp(0.,1.)).collect();
     }else{for x in curve{*x=(*x*f).clamp(0.,1.);}}
    }
   }else{
    // Compose legacy/full-range Lab input remapping before original input
    // curve. 16,385 evenly spaced samples, enough for the usual 4096-point
    // curves; native comparison below rejects excessive approximation error.
    let f=if target==2{1./LEGACY}else{LEGACY};
    for curve in &mut l.in_curves{
     *curve=(0..=16384).map(|i|interp(curve,(i as f64/16384.*f).clamp(0.,1.))).collect();
    }
   }
   if target==2{common_lengths(&mut l.in_curves)?;common_lengths(&mut l.out_curves)?;}
   let kind=if target==2{b"mft2"}else if forward{b"mAB "}else{b"mBA "};
   out.tags.insert(*k,crate::editing::encode(&l,kind)?);
  }else if is_text(k){out.tags.insert(*k,text_tag(&ascii_text(v)?,target,k==b"cprt"));}
  else if shared(k,v)||ink_metadata(p,k,v){out.tags.insert(*k,v.clone());}
 }
 for k in TRANSFORMS{if !out.tags.contains_key(&k){return Err("All six A2B/B2A intent tags are required".into())}}
 if !out.tags.contains_key(b"wtpt")||!out.tags.contains_key(b"desc"){return Err("Media white and profile description are required".into())}
 Ok((out,omitted))
}
fn check(source:&Profile,out:&Profile,target:u8,omitted:Vec<String>,max_de:f64,max_ink:f64,relative_absolute_only:bool)->R<Report>{
 if !max_de.is_finite()||!(0.05..=2.).contains(&max_de)||!max_ink.is_finite()||!(0.05..=2.).contains(&max_ink){return Err("Validation limits must be 0.05–2 ΔE00 / percentage points".into())}
 let n=crate::editing::editable_channel_count(source)?;
 if source.space()!=out.space()||source.pcs()!=out.pcs()||crate::editing::editable_channel_count(out)?!=n{return Err("Version conversion changed device signature or PCS".into())}
 let old=crate::print_space::bounds(source)?;let bounds=crate::print_space::bounds(out)?;
 // Preserve zero channels exactly; tolerate at most one 16-bit code for
 // other channel bounds and four codes for the conservative TAC bound.
 for j in 0..n{if bounds.channels[j]>old.channels[j]+if old.channels[j]==0.{0.}else{1./65535.}{return Err(format!("Conversion expands channel {} bound",j+1))}}
 if bounds.tac>old.tac+n as f64/65535.{return Err("Conversion expands conservative TAC bound".into())}
 let a=Native::new_icc(&source.bytes()?).map_err(|e|e.to_string())?;
 let b=Native::new_icc(&out.bytes()?).map_err(|e|e.to_string())?;
 let xyz=Native::new_xyz();let white=CIEXYZ{X:0.9642,Y:1.,Z:0.8249};
 let fi=crate::print_space::for_profile(&a)?;let fo=crate::print_space::for_profile(&b)?;
 if fi.channels!=n||fo.channels!=n{return Err("Native channel count mismatch".into())}
 let stride=2*n;
 let mut inks=crate::editing::device_samples(n);
 // Preserve dense legacy CMYK lattice in addition to bounded dynamic samples.
 if n==4{for i in 0..9usize.pow(4){let mut q=i;let mut v=[0u16;4];for j in (0..4).rev(){v[j]=(q%9*65535/8) as u16;q/=9;}for x in v{inks.extend_from_slice(&x.to_ne_bytes());}}}
 let count=inks.len()/stride;
 let mut pcs=Vec::new();
 for l in 0..17{for aa in 0..17{for bb in 0..17{
  let lab=CIELab{L:l as f64*100./16.,a:aa as f64*255./16.-128.,b:bb as f64*255./16.-128.};
  let t=lab.to_xyz(&white);let v=[t.X,t.Y,t.Z];if v.iter().all(|x|x.is_finite()&&*x>=0.&&*x<2.){pcs.push(v);}
 }}}
 // Dense neutral samples beyond the regular lattice.
 for i in 0..=100{let x=CIELab{L:i as f64,a:0.,b:0.}.to_xyz(&white);pcs.push([x.X,x.Y,x.Z]);}
 let mut checks=Vec::new();
 for (name,intent) in [("Perceptual",Intent::Perceptual),("Relative",Intent::RelativeColorimetric),("Saturation",Intent::Saturation),("Absolute",Intent::AbsoluteColorimetric)]{
  let f=|p:&Native|Transform::<u8,[f64;3]>::new_multiprofile(&[p,&xyz],fi.pixel,PixelFormat::XYZ_DBL,intent,Flags::NO_OPTIMIZE).map_err(|e|e.to_string());
  let r=|p:&Native|Transform::<[f64;3],u8>::new_multiprofile(&[&xyz,p],PixelFormat::XYZ_DBL,fo.pixel,intent,Flags::NO_OPTIMIZE).map_err(|e|e.to_string());
  let mut fa=vec![[0.;3];count];let mut fb=fa.clone();f(&a)?.transform_pixels(&inks,&mut fa);f(&b)?.transform_pixels(&inks,&mut fb);
  let mut ra=vec![0u8;pcs.len()*stride];let mut rb=ra.clone();r(&a)?.transform_pixels(&pcs,&mut ra);r(&b)?.transform_pixels(&pcs,&mut rb);
  let mut row=IntentCheck{name,max_de:0.,mean_de:0.,max_ink_pp:0.,max_tac_pp:0.,passed:false,required:!relative_absolute_only||matches!(intent,Intent::RelativeColorimetric|Intent::AbsoluteColorimetric)};
  for (x,y) in fa.into_iter().zip(fb){
   if x.iter().chain(&y).any(|v|!v.is_finite()){return Err("Nonfinite forward conversion".into())}
   let lab=|v:[f64;3]|CIEXYZ{X:v[0],Y:v[1],Z:v[2]}.to_lab(&white);
   let d=lab(x).cie2000_delta_e(&lab(y),1.,1.,1.);
   if !d.is_finite()||d<0.{return Err("Invalid forward Delta E".into())}row.max_de=row.max_de.max(d);row.mean_de+=d/count as f64;
  }
  for (x,y) in ra.chunks_exact(stride).zip(rb.chunks_exact(stride)){
   let mut sx=0u32;let mut sy=0u32;
   for j in 0..n{
    let a=u16::from_ne_bytes([x[2*j],x[2*j+1]]);let b=u16::from_ne_bytes([y[2*j],y[2*j+1]]);
    if old.channels[j]==0.&&b!=0{return Err(format!("Converted native transform reopened closed channel {}",j+1))}
    row.max_ink_pp=row.max_ink_pp.max(a.abs_diff(b) as f64*100./65535.);sx+=u32::from(a);sy+=u32::from(b);
   }
   row.max_tac_pp=row.max_tac_pp.max(sx.abs_diff(sy) as f64*100./65535.);
  }
  row.passed=row.max_de<=max_de&&row.max_ink_pp<=max_ink&&row.max_tac_pp<=n as f64*max_ink;
  if row.required&&!row.passed{return Err(format!("{name} intent equivalence failed: max ΔE00 {:.4} (limit {max_de:.2}), channel Δ {:.4} pp (limit {max_ink:.2}), TAC Δ {:.4} pp. No file saved.",row.max_de,row.max_ink_pp,row.max_tac_pp))}
  checks.push(row);
 }
 let mut colorant_metadata=Vec::new();
 for k in [b"clrt",b"clro"]{if let Some(v)=out.tags.get(k){
  if source.tags.get(k)!=Some(v){return Err("Colorant metadata changed during conversion".into())}
  colorant_metadata.push(String::from_utf8_lossy(k).into_owned());
 }}
 Ok(Report{target,relative_absolute_only,checks,omitted,forward_samples:count,reverse_samples:pcs.len(),bounds,channels:out.channels(),colorant_metadata})
}
pub fn convert_file(source:&Path,destination:&Path,target:u8,allow_omit:bool,max_de:f64,max_ink:f64,relative_absolute_only:bool)->R<Report>{
 let p=Profile::read(source)?;let (out,omitted)=build(&p,target,allow_omit)?;
 let report=check(&p,&out,target,omitted,max_de,max_ink,relative_absolute_only)?;
 out.save_new(destination)?;Ok(report)
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn real_lut_type_conversion_and_zero_k(){
  let p=crate::synthetic_cmyk_v4(true);
  let (v2,removed)=build(&p,2,false).unwrap();assert!(removed.is_empty());assert_eq!(v2.version_major(),2);
  for k in TRANSFORMS{assert!(v2.tags[&k].starts_with(b"mft2"));}
  assert_eq!(v2.verify([1.,1.,1.,0.]).unwrap()[3],0.);
  let (v4,_)=build(&v2,4,false).unwrap();
  assert!(v4.tags[b"A2B1"].starts_with(b"mAB "));assert!(v4.tags[b"B2A1"].starts_with(b"mBA "));
  assert_eq!(v4.verify([1.,1.,1.,0.]).unwrap()[3],0.);
  let native=Native::new_icc(&v2.bytes().unwrap()).unwrap();assert!(native.version()<3.);
  assert!(check(&p,&v2,2,vec![],0.5,0.5,false).is_err());
  let report=check(&p,&v2,2,vec![],0.5,0.5,true).unwrap();assert_eq!(report.checks.len(),4);assert!(report.checks.iter().filter(|i|i.required).all(|i|i.passed));assert!(report.checks.iter().any(|i|!i.required&&!i.passed));
  let report=check(&v2,&v4,4,vec![],0.5,0.5,true).unwrap();assert_eq!(report.checks.len(),4);
 }
 #[test]fn unknown_tags_require_permission(){
  let mut p=crate::synthetic_cmyk_v4(false);p.tags.insert(*b"DEVS",b"MSBN\0\0\0\0".to_vec());
  assert!(build(&p,2,false).is_err());
  let (v,omitted)=build(&p,2,true).unwrap();assert_eq!(omitted,vec!["DEVS"]);assert!(!v.tags.contains_key(b"DEVS"));assert!(p.tags.contains_key(b"DEVS"));
  p.tags.insert(*b"D2B0",b"mpet\0\0\0\0".to_vec());assert!(build(&p,2,true).is_err());
 }
 #[test]fn legacy_identity_pcs_curve_is_supported(){
  let (mut v2,_)=build(&crate::synthetic_cmyk_v4(true),2,false).unwrap();
  for k in [b"A2B0",b"A2B1",b"A2B2"]{
   let mut l=Lut::parse(&v2.tags[k]).unwrap();
   for j in 0..3{for n in l.clut.chunks_exact_mut(3){n[j]*=LEGACY;}}
   l.out_curves=vec![vec![0.,1.];3];
   v2.tags.insert(*k,crate::editing::encode(&l,b"mft2").unwrap());
  }
  let (v4,_)=build(&v2,4,false).unwrap();
  let l=Lut::parse(&v4.tags[b"A2B1"]).unwrap();assert!((interp(&l.out_curves[0],0.5)-0.5/LEGACY).abs()<0.00003);
 }
 #[test]fn unequal_curve_lengths_retain_knots(){
  let mut c=vec![vec![0.,0.1,1.],vec![0.,1.],vec![0.,0.2,0.8,1.]];
  common_lengths(&mut c).unwrap();assert_eq!(c[0].len(),7);assert!((c[0][3]-0.1).abs()<1e-12);assert!((c[2][2]-0.2).abs()<1e-12);
 }
 #[test]fn text_round_trip(){
  for copyright in [false,true]{let s="Copyright test";let v=text_tag(s,4,copyright);assert_eq!(ascii_text(&v).unwrap(),s);assert_eq!(ascii_text(&text_tag(s,2,copyright)).unwrap(),s);}
 }
 #[test]fn failed_validation_or_existing_path_does_not_publish(){
  let d=tempfile::tempdir().unwrap();let a=d.path().join("a.icc");let b=d.path().join("b.icc");crate::synthetic_cmyk_v4(true).save_new(&a).unwrap();
  assert!(convert_file(&a,&b,2,false,0.,0.5,false).is_err());assert!(!b.exists());assert!(convert_file(&a,&a,2,false,0.5,0.5,true).is_err());
 }
}

#[cfg(test)]
mod dynamic_tests{
 use super::*;
 #[test]fn nclr_structural_roundtrip_preserves_device_axes_and_zero_ink(){
  for n in [3,4,6,8,15]{
   let mut p=crate::print_space::fixture(n);
   let mut table=b"clrt\0\0\0\0".to_vec();put32(&mut table,n as u32);
   for i in 0..n{let mut name=format!("Ink {}",i+1).into_bytes();name.resize(32,0);table.extend(name);for x in [32768u16,32896,32896]{put16(&mut table,x);}}
   p.tags.insert(*b"clrt",table);
   let mut order=b"clro\0\0\0\0".to_vec();put32(&mut order,n as u32);order.extend((0..n as u8).rev());p.tags.insert(*b"clro",order);
   let (v2,omitted)=build(&p,2,false).unwrap();assert!(omitted.is_empty());assert_eq!(v2.space(),p.space());assert_eq!(v2.channels(),p.channels());
   let (v4,_)=build(&v2,4,false).unwrap();
   for k in TRANSFORMS{assert!(v2.tags[&k].starts_with(b"mft2"));let a=Lut::parse(&p.tags[&k]).unwrap();let b=Lut::parse(&v4.tags[&k]).unwrap();assert_eq!((a.inputs,a.outputs,a.grid),(b.inputs,b.outputs,b.grid));assert_eq!(a.clut,b.clut);}
   for k in [b"clrt",b"clro"]{assert_eq!(p.tags[k],v4.tags[k]);}
   if n>3{assert_eq!(crate::print_space::bounds(&v4).unwrap().channels[n-1],0.);}
  }
 }
 #[test]fn native_six_channel_compatibility_report(){
  let p=crate::print_space::fixture(6);
  let (v2,_)=build(&p,2,false).unwrap();let r=check(&p,&v2,2,vec![],0.5,0.5,true).unwrap();
  assert_eq!(r.channels.len(),6);assert_eq!(r.bounds.channels[5],0.);assert_eq!(r.checks.len(),4);assert!(r.forward_samples>=4096);
  let (v4,_)=build(&v2,4,false).unwrap();assert!(check(&v2,&v4,4,vec![],0.5,0.5,true).is_ok());
 }
 #[test]fn malformed_colorant_metadata_needs_consent(){
  let mut p=crate::print_space::fixture(6);p.tags.insert(*b"clro",b"clro\0\0\0\0\0\0\0\x06\0\0\0\0\0\0".to_vec());
  assert!(build(&p,2,false).is_err());let (q,omitted)=build(&p,2,true).unwrap();assert!(omitted.contains(&"clro".to_owned()));assert!(!q.tags.contains_key(b"clro"));
 }
}
