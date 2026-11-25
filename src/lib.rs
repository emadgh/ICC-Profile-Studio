mod icc_curve;
pub mod advanced_lut;
pub mod multichannel;
pub mod rtl;
pub mod print_space;
pub mod i18n;
pub mod system_profiles;
pub mod versioning;
pub mod gamut_surface;
pub mod gamut_raster;
pub mod validation;
pub mod preferences;
pub mod reports;
pub mod optimizer;
pub mod lab;
pub mod profiling_target;
pub mod characterization;
pub mod characterization_tools;
mod characterization_cgats;
mod spectral_colorimetry;
pub mod cie_spectral;
pub mod device_link;
mod chart_tiff;
mod separated_tiff;
pub mod correction;
pub mod gradient;
pub mod gamut;
pub mod editing;
pub mod comparison;
pub mod backend;
pub mod inspection;
use std::collections::BTreeMap;
use std::{fs,io::Write,path::Path};

type R<T> = Result<T,String>;
fn u16at(b:&[u8],p:usize)->R<u16>{Ok(u16::from_be_bytes(b.get(p..p+2).ok_or("Truncated ICC")?.try_into().unwrap()))}
fn u32at(b:&[u8],p:usize)->R<u32>{Ok(u32::from_be_bytes(b.get(p..p+4).ok_or("Truncated ICC")?.try_into().unwrap()))}
fn put16(b:&mut Vec<u8>,v:u16){b.extend(v.to_be_bytes());}
fn put32(b:&mut Vec<u8>,v:u32){b.extend(v.to_be_bytes());}
fn signature(b:&[u8])->String{String::from_utf8_lossy(b).into_owned()}
#[derive(Clone)]
pub struct Profile {header:Vec<u8>, tags:BTreeMap<[u8;4],Vec<u8>>}
impl Profile {
 pub fn read(p:&Path)->R<Self>{
  let meta=fs::metadata(p).map_err(|e|e.to_string())?;
  if meta.len()>128*1024*1024{return Err("ICC larger than 128 MiB".into())}
  Self::parse(&fs::read(p).map_err(|e|e.to_string())?)
 }
 pub fn parse(b:&[u8])->R<Self>{
  if b.len()<132 || &b[36..40]!=b"acsp"{return Err("Not an ICC profile".into())}
  let size=u32at(b,0)? as usize;
  if size<132 || size>b.len(){return Err("Invalid ICC size".into())}
  let n=u32at(b,128)? as usize;
  if n>4096 || 132+12*n>size{return Err("Invalid ICC tag directory".into())}
  let mut tags=BTreeMap::new();
  for i in 0..n {
   let p=132+i*12;let key:[u8;4]=b[p..p+4].try_into().unwrap();
   let o=u32at(b,p+4)? as usize;let l=u32at(b,p+8)? as usize;
   if o<132+12*n || l<8 || o.checked_add(l).filter(|v|*v<=size).is_none(){return Err("Invalid tag bounds".into())}
   if tags.insert(key,b[o..o+l].to_vec()).is_some(){return Err("Duplicate ICC tag".into())}
  }
  Ok(Self{header:b[..128].to_vec(),tags})
 }
 pub fn space(&self)->String{signature(&self.header[16..20])}
 pub fn pcs(&self)->String{signature(&self.header[20..24])}
 pub fn version_major(&self)->u8{self.header[8]}
 pub fn cmyk(&self)->bool{&self.header[16..20]==b"CMYK"}
 pub fn standard_channels(&self)->Vec<String>{
  let s=self.space();
  let names:Vec<String>=match s.as_str(){
   "CMYK"=>["Cyan","Magenta","Yellow","Black"].iter().map(|s|s.to_string()).collect(),
   "CMY "=>["Cyan","Magenta","Yellow"].iter().map(|s|s.to_string()).collect(),
   "RGB "=>["Red","Green","Blue"].iter().map(|s|s.to_string()).collect(),
   "GRAY"=>vec!["Gray".into()],
   _=>{let n=if s.ends_with("CLR"){s.chars().next().and_then(|c|c.to_digit(16)).unwrap_or(0)}else{0};(1..=n).map(|i|format!("Channel {i}")).collect()}
  };
  names
 }
 /// Colorant table entries in device channel order; names are not inferred
 /// from the displayed color. PCS coordinates are not measured ink spectra.
 pub fn colorants(&self)->Vec<Option<(String,[u16;3])>>{
  let n=self.standard_channels().len();let mut result=vec![None;n];
  if let Some(t)=self.tags.get(b"clrt"){
   if t.starts_with(b"clrt")&&u32at(t,8).ok()==Some(n as u32)&&t.len()>=12+38*n{
    for (i,out) in result.iter_mut().enumerate(){
     let o=12+i*38;let v=&t[o..o+32];let end=v.iter().position(|&x|x==0).unwrap_or(v.len());
     let name=String::from_utf8_lossy(&v[..end]).trim().to_owned();
     if !name.is_empty()&&!name.chars().any(char::is_control){
      *out=Some((name,[u16at(t,o+32).unwrap(),u16at(t,o+34).unwrap(),u16at(t,o+36).unwrap()]));
     }
    }
   }
  }
  result
 }
 pub fn channels(&self)->Vec<String>{
  self.standard_channels().into_iter().zip(self.colorants()).map(|(fallback,v)|v.map(|(name,_)|name).unwrap_or(fallback)).collect()
 }
 /// The standard CMYK swatches are UI conventions. The optional v4
 /// colorant-table swatch is a clipped sRGB rendering of stored PCS data.
 /// No fabricated ink hues for unlabelled nCLR channels.
 pub fn channel_swatch(&self,index:usize,embedded:bool)->u32{
  if embedded&&self.version_major()==4&&&self.header[12..16]==b"prtr"{
   if let Some(Some((_,v)))=self.colorants().get(index){
    let lab=if self.pcs()=="Lab "{Some([v[0] as f32*100./65535.,v[1] as f32/257.-128.,v[2] as f32/257.-128.])}
     else if self.pcs()=="XYZ "{
      use lcms2::{CIEXYZ,CIEXYZExt};
      let w=CIEXYZ{X:0.9642,Y:1.,Z:0.8249};
      let c=CIEXYZ{X:v[0] as f64/32768.,Y:v[1] as f64/32768.,Z:v[2] as f64/32768.}.to_lab(&w);
      Some([c.L as f32,c.a as f32,c.b as f32])
     }else{None};
    if let Some(l)=lab{let [r,g,b]=crate::gamut_raster::lab_rgb(l);return (u32::from(r)<<16)|(u32::from(g)<<8)|u32::from(b);}
   }
  }
  if self.cmyk(){[0x00b7e5,0xe33b91,0xe8c44d,0x161616].get(index).copied().unwrap_or(0x858585)}else{0x858585}
 }
 pub fn check(&self)->R<()>{
  if &self.header[12..16]!=b"prtr" || self.channels().len()!=4{return Err("Only four-channel printer ICC profiles are supported".into())}
  if !self.cmyk() && self.space()!="4CLR"{return Err("Expected CMYK or 4CLR".into())}
  if self.tags.keys().any(|k|k.starts_with(b"B2D")){return Err("Unsupported alternate B2D output transforms".into())}
  Ok(())
 }
 pub fn bytes(&self)->R<Vec<u8>>{
  let mut out=self.header.clone();out[84..100].fill(0);
  put32(&mut out,self.tags.len() as u32);out.resize(132+12*self.tags.len(),0);
  for (i,(key,val)) in self.tags.iter().enumerate(){
   while out.len()%4!=0{out.push(0)}
   let o=u32::try_from(out.len()).map_err(|_|"ICC too big")?;let l=val.len() as u32;let p=132+i*12;
   out[p..p+4].copy_from_slice(key);out[p+4..p+8].copy_from_slice(&o.to_be_bytes());out[p+8..p+12].copy_from_slice(&l.to_be_bytes());out.extend(val);
  }
  while out.len()%4!=0{out.push(0)}
  let n=out.len() as u32;out[..4].copy_from_slice(&n.to_be_bytes());Ok(out)
 }
 pub fn save_new(&self,p:&Path)->R<()>{
  let b=self.bytes()?;let mut f=fs::OpenOptions::new().write(true).create_new(true).open(p).map_err(|e|e.to_string())?;
  if let Err(e)=f.write_all(&b).and_then(|_|f.sync_all()){drop(f);let _=fs::remove_file(p);return Err(e.to_string())}Ok(())
 }
 pub fn describe(&mut self,name:&str){
  let mut t=Vec::new();
  if self.header[8]>=4{
   t.extend(b"mluc\0\0\0\0");put32(&mut t,1);put32(&mut t,12);t.extend(b"enUS");
   let chars:Vec<u16>=name.encode_utf16().collect();put32(&mut t,(2*chars.len()) as u32);put32(&mut t,28);for c in chars{put16(&mut t,c)}
  }else{
   t.extend(b"desc\0\0\0\0");let ascii:Vec<u8>=name.chars().map(|c|if c.is_ascii()&&!c.is_control(){c as u8}else{b'_'}).collect();
   put32(&mut t,(ascii.len()+1) as u32);t.extend(ascii);t.push(0);put32(&mut t,0);put32(&mut t,0);put16(&mut t,0);t.push(0);t.extend([0;67]);
  }
  self.tags.insert(*b"desc",t);
 }
 pub fn clean_stale(&mut self){for k in [b"gamt",b"pre0",b"pre1",b"pre2",b"gbd0",b"gbd1",b"gbd2",b"gbd3"]{self.tags.remove(k);}}
 pub fn estimated_tac_percent(&self)->Option<f64>{
  let lut=Lut::parse(self.tags.get(b"B2A1")?).ok()?;
  if lut.inputs!=3||lut.outputs!=4{return None}
  let peak=lut.clut.chunks_exact(4).map(|node|(0..4).map(|j|interp(&lut.out_curves[j],node[j])).sum::<f64>()).fold(0.0_f64,f64::max);
  Some((100.*peak).floor().clamp(0.,400.))
 }
 pub fn verify(&self,caps:[f64;4])->R<[f64;4]>{
  self.check()?;let mut max:[f64;4]=[0.;4];
  for k in [b"B2A0",b"B2A1",b"B2A2"]{
   let t=self.tags.get(k).ok_or("Missing rebuilt B2A intent")?;
   if Lut::parse(t).is_err()&&t.starts_with(b"mBA "){
    let b=crate::print_space::bounds(self)?;
    let result:[f64;4]=b.channels.try_into().map_err(|_|"Unexpected channel count")?;
    for j in 0..4{if result[j]>caps[j]+1e-10{return Err(format!("Channel {} exceeds limit",j+1))}}
    return Ok(result)
   }
   let lut=Lut::parse(t)?;
   if lut.inputs!=3 || lut.outputs!=4{return Err("Unexpected output LUT dimensions".into())}
   for j in 0..4{
    // Interpolated CLUT outputs stay within node extrema. Bound the
    // post-CLUT curve over that entire interval, not just sampled Lab values.
    let (mut lo,mut hi)=(1.0_f64,0.0_f64);
    for node in lut.clut.chunks_exact(4){lo=lo.min(node[j]);hi=hi.max(node[j]);}
    let curve=&lut.out_curves[j];let scale=(curve.len()-1) as f64;
    let mut peak=interp(curve,lo).max(interp(curve,hi));
    for (i,&v) in curve.iter().enumerate(){let x=i as f64/scale;if x>=lo && x<=hi{peak=peak.max(v);}}
    max[j]=max[j].max(peak);
   }
  }
  for j in 0..4{if max[j]>caps[j]+1e-10{return Err(format!("Channel {} exceeds limit: {:.5}% > {:.5}%",j+1,100.*max[j],100.*caps[j]))}}
  Ok(max)
 }
 pub fn custom_rebuild(&self,caps:[f64;4],tac:f64,grid:usize,progress:impl Fn(String))->R<Self>{
  self.check()?;
  if self.pcs()!="Lab " || !(self.header[8]==2||self.header[8]==4){return Err("Custom engine requires ICC v2/v4 Lab PCS".into())}
  let forward=self.tags.get(b"A2B1").ok_or("Custom engine requires A2B1")?;
  if self.tags.keys().any(|k|k.starts_with(b"D2B")){return Err("Alternate D2B transforms are not supported by rebuilding".into())}
  if self.header[8]==4 && !forward.starts_with(b"mAB "){return Err("ICC v4 custom engine requires A2B1 mAB".into())}
  if self.header[8]==2 && !forward.starts_with(b"mft2")&&!forward.starts_with(b"mft1"){return Err("ICC v2 custom engine requires A2B1 mft1 or mft2".into())}
  // Evaluate the original native pipeline; retain its exact encoded curves,
  // unequal CLUT axes and interpolation rather than sampling and re-encoding
  // the forward model. The new reverse LUT remains bounded and independent.
  let native=lcms2::Profile::new_icc(&self.bytes()?).map_err(|e|format!("LittleCMS: {e}"))?;
  let lcms2::Tag::Pipeline(lut)=native.read_tag(lcms2::TagSignature::AToB1Tag)else{return Err("LittleCMS could not decode A2B1".into())};
  if lut.input_channels()!=4||lut.output_channels()!=3{return Err("Expected A2B1 4 -> 3".into())}
  let predict=|x:[f64;4]|->R<[f64;3]>{
   let mut y=[0f32;3];lut.eval(&x.map(|v|v as f32),&mut y);
   if y.iter().any(|v|!v.is_finite()||*v< -0.00002||*v>1.00002){return Err("Invalid native A2B1 normalized output".into())}
   Ok(y.map(|v|f64::from(v).clamp(0.,1.)))
  };
  if !(9..=33).contains(&grid) || !(0.0..=4.0).contains(&tac) || caps.iter().any(|v|!v.is_finite()||!(0.0..=1.0).contains(v)){return Err("Invalid settings".into())}
  let mut seeds=Vec::new();
  for i in 0..625usize{
   let mut q=i;let mut x=[0.;4];for j in (0..4).rev(){x[j]=caps[j]*(q%5) as f64/4.;q/=5;}
   if valid(x,caps,tac){seeds.push((x,predict(x)?)); }
  }
  let mut table=Vec::with_capacity(grid.pow(3)*8);
  let denom=(grid-1) as f64;
  let scale=if self.header[8]==2{[100.*65535./65280.,255.*65535./65280.,255.*65535./65280.]}else{[100.,255.,255.]};
  let distance=|a:[f64;3],b:[f64;3]|->f64{(0..3).map(|j|((a[j]-b[j])*scale[j]).powi(2)).sum()};
  for l in 0..grid{
   progress(format!("Inverting A2B1: slice {}/{}",l+1,grid));
   for a in 0..grid{for b in 0..grid{
    let target=[l as f64/denom,a as f64/denom,b as f64/denom];
    let mut best=seeds[0].0;let mut cost=f64::INFINITY;
    for &(x,lab) in &seeds{let d=distance(lab,target);if d<cost{best=x;cost=d}}
    let mut step=0.25;
    for _ in 0..10{
     for _ in 0..4{
      let prev=best;
      for j in 0..4{for dir in [-1.,1.]{
       let mut x=best;x[j]=(x[j]+dir*step).clamp(0.,caps[j]);
       if valid(x,caps,tac){let d=distance(predict(x)?,target);if d+1e-12<cost{best=x;cost=d;}}
      }}
      for j in 0..4{for k in j+1..4{for dir in [-1.,1.]{
       let mut x=best;let delta=dir*step;x[j]+=delta;x[k]-=delta;
       if valid(x,caps,tac){let d=distance(predict(x)?,target);if d+1e-12<cost{best=x;cost=d;}}
      }}}
      if prev==best{break}
     }
     step*=0.5;
    }
    for v in best{put16(&mut table,(v.clamp(0.,1.)*65535.).floor() as u16);}
   }}
  }
  let mut t=Vec::new();
  if self.header[8]==4 {t=make_mba(grid,&table);} else {
  t.extend(b"mft2\0\0\0\0");t.extend([3,4,grid as u8,0]);
  for i in 0..9{put32(&mut t,if i%4==0{65536}else{0})}
  put16(&mut t,2);put16(&mut t,2);
  for _ in 0..3{put16(&mut t,0);put16(&mut t,65535)}
  t.extend(table);
  for _ in 0..4{put16(&mut t,0);put16(&mut t,65535)}
  }
  let mut p=self.clone();
  for k in [b"B2A0",b"B2A1",b"B2A2"]{p.tags.insert(*k,t.clone());}
  p.clean_stale();p.verify(caps)?;Ok(p)
 }
}
fn valid(x:[f64;4],caps:[f64;4],tac:f64)->bool{(0..4).all(|i|x[i]>=0.&&x[i]<=caps[i])&&x.iter().sum::<f64>()<=tac+1e-12}
fn interp(curve:&[f64],x:f64)->f64{
 let pos=x.clamp(0.,1.)*(curve.len()-1) as f64;let i=pos.floor() as usize;let j=(i+1).min(curve.len()-1);curve[i]+(curve[j]-curve[i])*(pos-i as f64)
}
struct Lut {inputs:usize,outputs:usize,grid:usize,in_curves:Vec<Vec<f64>>,clut:Vec<f64>,out_curves:Vec<Vec<f64>>}
impl Lut{
 fn parse(t:&[u8])->R<Self>{
  if t.starts_with(b"mAB ")||t.starts_with(b"mBA "){return Self::parse_v4(t)}
  if t.len()<52{return Err("Truncated LUT".into())}
  let (width,mut off,ni,no)=match &t[..4]{b"mft1"=>(1,48,256,256),b"mft2"=>(2,52,u16at(t,48)? as usize,u16at(t,50)? as usize),_=>return Err("Only mft1/mft2 output LUTs can be verified".into())};
  let (inputs,outputs,grid)=(t[8] as usize,t[9] as usize,t[10] as usize);
  if inputs==0||inputs>15||outputs==0||outputs>15||grid<2||ni<2||no<2{return Err("Invalid LUT dimensions".into())}
  // Matrix must be identity: we only support plain CLUT tables here.
  for i in 0..9{if u32at(t,12+i*4)?!=if i%4==0{65536}else{0}{return Err("Nonidentity LUT matrix is unsupported".into())}}
  let count=grid.checked_pow(inputs as u32).and_then(|v|v.checked_mul(outputs)).ok_or("LUT overflow")?;
  let total=inputs*ni+count+outputs*no;
  if total>16_000_000||off+total*width>t.len(){return Err("Invalid LUT data length".into())}
  let mut take=|n:usize|->R<Vec<f64>>{let mut v=Vec::with_capacity(n);for _ in 0..n{v.push(if width==1{t[off] as f64/255.}else{u16at(t,off)? as f64/65535.});off+=width;}Ok(v)};
  let mut in_curves=Vec::new();for _ in 0..inputs{in_curves.push(take(ni)?)}
  let clut=take(count)?;
  let mut out_curves=Vec::new();for _ in 0..outputs{out_curves.push(take(no)?)}
  Ok(Self{inputs,outputs,grid,in_curves,clut,out_curves})
 }
 fn parse_v4(t:&[u8])->R<Self>{
  if t.len()<32{return Err("Truncated ICC v4 LUT".into())}
  let inputs=t[8] as usize;let outputs=t[9] as usize;
  if !(1..=15).contains(&inputs)||!(1..=15).contains(&outputs){return Err("Invalid v4 LUT channel count".into())}
  let b=u32at(t,12)? as usize;let matrix=u32at(t,16)? as usize;
  let m=u32at(t,20)? as usize;let cl=u32at(t,24)? as usize;let a=u32at(t,28)? as usize;
  if matrix!=0||m!=0{return Err("v4 LUT with matrix/M curves is not supported by this engine".into())}
  if cl<32||cl%4!=0||cl.checked_add(20).filter(|&v|v<=t.len()).is_none(){return Err("Invalid v4 CLUT offset".into())}
  let grid=t[cl] as usize;
  if grid<2||t[cl..cl+inputs].iter().any(|&v|v as usize!=grid){return Err("Only equal-grid v4 CLUTs are supported".into())}
  let width=t[cl+16] as usize;
  if width!=1&&width!=2{return Err("Unsupported v4 CLUT precision".into())}
  let count=grid.checked_pow(inputs as u32).and_then(|v|v.checked_mul(outputs)).ok_or("CLUT overflow")?;
  let end=(cl+20).checked_add(count.checked_mul(width).ok_or("CLUT overflow")?).ok_or("CLUT overflow")?;
  if count>16_000_000||end>t.len(){return Err("Truncated or oversized v4 CLUT".into())}
  let clut=if width==1{t[cl+20..end].iter().map(|&v|v as f64/255.).collect()}else{t[cl+20..end].chunks_exact(2).map(|v|u16::from_be_bytes([v[0],v[1]]) as f64/65535.).collect()};
  let (input_offset,output_offset)=if t.starts_with(b"mAB "){(a,b)}else{(b,a)};
  let in_curves=read_curve_set(t,input_offset,inputs)?;
  let out_curves=read_curve_set(t,output_offset,outputs)?;
  Ok(Self{inputs,outputs,grid,in_curves,clut,out_curves})
 }
 fn eval4(&self,x:[f64;4])->[f64;3]{
  let mut lo=[0usize;4];let mut hi=[0usize;4];let mut frac=[0.;4];
  for j in 0..4{let p=interp(&self.in_curves[j],x[j])*(self.grid-1) as f64;lo[j]=p.floor() as usize;hi[j]=(lo[j]+1).min(self.grid-1);frac[j]=p-lo[j] as f64;}
  let mut y=[0.;3];
  for mask in 0..16{let mut w=1.;let mut idx=0;for j in 0..4{if mask&(1<<j)!=0{w*=frac[j];idx=idx*self.grid+hi[j]}else{w*=1.-frac[j];idx=idx*self.grid+lo[j]}}
   if w>0.{for j in 0..3{y[j]+=w*self.clut[idx*3+j]}}
  }
  for j in 0..3{y[j]=interp(&self.out_curves[j],y[j])}y
 }
}

// ICC v4 curveType sets are four-byte aligned. The production profile
// uses sampled curves (4096 entries) and two-point identity curves.
fn read_curve_set(t:&[u8],mut off:usize,n:usize)->R<Vec<Vec<f64>>>{
 if off==0{return Ok(vec![vec![0.,1.];n])}
 if off<32||off%4!=0{return Err("Invalid v4 curve offset".into())}
 let mut out=Vec::new();
 for _ in 0..n{
  if off.checked_add(12).filter(|&v|v<=t.len()).is_none(){return Err("Truncated v4 curve".into())}
  if &t[off..off+4]!=b"curv"{return Err("Only sampled v4 curveType (curv) is supported; parametric curves are not approximated".into())}
  let count=u32at(t,off+8)? as usize;
  if count==1{return Err("Gamma-encoded single-entry curves are unsupported".into())}
  let end=(off+12).checked_add(count.checked_mul(2).ok_or("Curve size overflow")?).ok_or("Curve size overflow")?;
  if end>t.len()||count>65536{return Err("Invalid v4 curve size".into())}
  let curve=if count==0{vec![0.,1.]}else{t[off+12..end].chunks_exact(2).map(|v|u16::from_be_bytes([v[0],v[1]]) as f64/65535.).collect()};
  out.push(curve);off=(end+3)&!3;
 }
 Ok(out)
}
fn identity_curves(t:&mut Vec<u8>,n:usize){
 for _ in 0..n{t.extend(b"curv\0\0\0\0");put32(t,2);put16(t,0);put16(t,65535);}
}
fn make_mba(grid:usize,table:&[u8])->Vec<u8>{
 let mut t=b"mBA \0\0\0\0".to_vec();
 t.extend([3,4,0,0]);put32(&mut t,32);put32(&mut t,0);put32(&mut t,0);
 let cl=32+3*16;let a=(cl+20+table.len()+3)&!3;
 put32(&mut t,cl as u32);put32(&mut t,a as u32);
 identity_curves(&mut t,3);
 t.extend([grid as u8;3]);t.extend([0;13]);t.extend([2,0,0,0]);
 t.extend(table);while t.len()%4!=0{t.push(0)}
 identity_curves(&mut t,4);t
}

#[cfg(test)]
mod tests{
 use super::*;
 fn fixture()->Profile{
  let mut header=vec![0;128];header[8]=2;header[12..16].copy_from_slice(b"prtr");header[16..20].copy_from_slice(b"CMYK");header[20..24].copy_from_slice(b"Lab ");header[36..40].copy_from_slice(b"acsp");
  let mut t=b"mft2\0\0\0\0".to_vec();t.extend([4,3,2,0]);for i in 0..9{put32(&mut t,if i%4==0{65536}else{0})}put16(&mut t,2);put16(&mut t,2);
  for _ in 0..4{put16(&mut t,0);put16(&mut t,65535)}
  for i in 0..16{let c=(i>>3)&1;let m=(i>>2)&1;let y=(i>>1)&1;let k=i&1;put16(&mut t,(65535-10000*(c+m+y+k)) as u16);put16(&mut t,(20000+20000*m) as u16);put16(&mut t,(20000+20000*y) as u16)}
  for _ in 0..3{put16(&mut t,0);put16(&mut t,65535)}
  let mut tags=BTreeMap::new();tags.insert(*b"A2B1",t);Profile{header,tags}
 }
 #[test]fn v4_mab_mba_rebuild(){
  let mut p=fixture();p.header[8]=4;p.header[9]=0x30;
  let old=p.tags.get(b"A2B1").unwrap();
  // mAB: 4 input curves, 17-style 4D CLUT, 3 output curves.
  let mut t=b"mAB \0\0\0\0".to_vec();t.extend([4,3,0,0]);
  let cl=32+3*16;let a=cl+20+16*3*2;
  put32(&mut t,32);put32(&mut t,0);put32(&mut t,0);put32(&mut t,cl as u32);put32(&mut t,a as u32);
  identity_curves(&mut t,3);t.extend([2;4]);t.extend([0;12]);t.extend([2,0,0,0]);
  t.extend(&old[68..68+96]);identity_curves(&mut t,4);
  p.tags.insert(*b"A2B1",t);
  let caps=[1.,1.,1.,0.];let result=p.custom_rebuild(caps,3.,9,|_|{}).unwrap();
  assert_eq!(result.version_major(),4);assert!(result.tags[b"B2A1"].starts_with(b"mBA "));
  assert_eq!(Profile::parse(&result.bytes().unwrap()).unwrap().verify(caps).unwrap()[3],0.);
 }
 #[test]fn native_rebuild_keeps_parametric_forward(){
  let mut p=synthetic_cmyk_v4(true);
  let mut a=p.tags[b"A2B1"].clone();
  let offset=a.len() as u32;a[28..32].copy_from_slice(&offset.to_be_bytes());
  for _ in 0..4{a.extend(b"para\0\0\0\0");put16(&mut a,0);put16(&mut a,0);put32(&mut a,2*65536);}
  p.tags.insert(*b"A2B1",a.clone());
  assert!(Lut::parse(&a).is_err()); // old strict sampled parser intentionally does not approximate
  let out=p.custom_rebuild([1.,1.,1.,0.],1.,9,|_|{}).unwrap();
  assert_eq!(out.tags[b"A2B1"],a);
  let (b,n)=crate::validation::verify_output(&out,[1.,1.,1.,0.],Some(1.)).unwrap();
  assert_eq!(b.channels[3],0.);assert_eq!(n.channels[3],0.);
 }
 #[test]fn native_rebuild_keeps_unequal_forward_axes(){
  let mut p=synthetic_cmyk_v4(true);
  let mut a=b"mAB \0\0\0\0".to_vec();a.extend([4,3,0,0]);
  let cl=32+3*16;let data=2*3*2*2*3*2;
  for v in [32,0,0,cl as u32,(cl+20+data) as u32]{put32(&mut a,v);}
  identity_curves(&mut a,3);a.extend([2,3,2,2]);a.extend([0;12]);a.extend([2,0,0,0]);
  for c in 0..2{for m in 0..3{for y in 0..2{for k in 0..2{
   for v in [1.-0.15*(c as f64+m as f64/2.+y as f64+k as f64),0.4+0.1*m as f64,0.4+0.1*y as f64]{put16(&mut a,(v*65535.).round() as u16);}
  }}}}
  identity_curves(&mut a,4);p.tags.insert(*b"A2B1",a.clone());
  assert!(Lut::parse(&a).is_err());
  let out=p.custom_rebuild([1.,1.,1.,0.],1.,9,|_|{}).unwrap();
  assert_eq!(out.tags[b"A2B1"],a);assert_eq!(out.verify([1.,1.,1.,0.]).unwrap()[3],0.);
 }
 #[test]fn rejects_bad_header(){assert!(Profile::parse(&[0;132]).is_err());}
 #[test]fn channel_names(){assert_eq!(fixture().channels(),vec!["Cyan","Magenta","Yellow","Black"]);}
 #[test]fn constrained_rebuild(){let p=fixture();let caps=[0.0,1.,0.10,0.05];let out=p.custom_rebuild(caps,2.,9,|_|{}).unwrap();let parsed=Profile::parse(&out.bytes().unwrap()).unwrap();let max=parsed.verify(caps).unwrap();assert_eq!(max[0],0.);assert!(max[2]<=0.10);assert!(max[3]<=0.05);}
}

// A deliberately synthetic ICC used only to exercise actual LCMS transforms.
// Not a measured printer characterization and never distributed as a profile.
#[cfg(test)]
pub(crate) fn synthetic_cmyk_v4(k_zero:bool)->Profile{
 let mut header=vec![0;128];
 header[8]=4;header[9]=0x30;header[12..16].copy_from_slice(b"prtr");header[16..20].copy_from_slice(b"CMYK");header[20..24].copy_from_slice(b"Lab ");header[36..40].copy_from_slice(b"acsp");
 for (j,v) in [0x0000f6d6u32,0x00010000,0x0000d32d].into_iter().enumerate(){header[68+4*j..72+4*j].copy_from_slice(&v.to_be_bytes());}
 let mut white=b"XYZ \0\0\0\0".to_vec();for v in [0x0000f6d6,0x00010000,0x0000d32d]{put32(&mut white,v);}
 let mut forward=b"mAB \0\0\0\0".to_vec();forward.extend([4,3,0,0]);
 let cl=32+3*16;let a=cl+20+16*3*2;
 for v in [32,0,0,cl as u32,a as u32]{put32(&mut forward,v);}
 identity_curves(&mut forward,3);forward.extend([2;4]);forward.extend([0;12]);forward.extend([2,0,0,0]);
 for i in 0..16{let c=(i>>3)&1;let m=(i>>2)&1;let y=(i>>1)&1;let k=i&1;
  for v in [1.-0.18*(c+m+y+k) as f64,0.50+0.18*(m as f64-c as f64),0.50+0.18*(y as f64-c as f64)]{put16(&mut forward,(v*65535.).round() as u16);}
 }
 identity_curves(&mut forward,4);
 let mut reverse=Vec::new();
 for l in 0..2{for a in 0..2{for b in 0..2{
  for v in [0.30*(1-l) as f64,0.25*a as f64,0.25*b as f64,if k_zero{0.}else{0.20*(1-l) as f64}]{put16(&mut reverse,(65535.*v) as u16);}
 }}}
 let reverse=make_mba(2,&reverse);
 let mut p=Profile{header,tags:BTreeMap::new()};p.describe("Synthetic CI fixture (not a printer characterization)");
 p.tags.insert(*b"wtpt",white);
 for key in [b"A2B0",b"A2B1",b"A2B2"]{p.tags.insert(*key,forward.clone());}
 for key in [b"B2A0",b"B2A1",b"B2A2"]{p.tags.insert(*key,reverse.clone());}
 p
}

#[cfg(test)]mod channel_display_tests{
 use super::*;
 #[test]fn stored_names_do_not_change_standard_mapping(){
  let mut p=synthetic_cmyk_v4(true);let mut table=b"clrt\0\0\0\0".to_vec();put32(&mut table,4);
  for name in ["Blue","Brown","Beige","Black"]{let mut v=name.as_bytes().to_vec();v.resize(32,0);table.extend(v);for x in [32768,32768,32768]{put16(&mut table,x);}}
  p.tags.insert(*b"clrt",table);
  assert_eq!(p.channels()[0],"Blue");assert_eq!(p.standard_channels()[0],"Cyan");assert_eq!(p.channel_swatch(0,false),0x00b7e5);assert_ne!(p.channel_swatch(0,true),0x00b7e5);
  p.tags.remove(b"clrt");assert_eq!(p.channels(),p.standard_channels());
 }
}
