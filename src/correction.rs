//! Selective LCh correction and bounded separation smoothing.
//! LittleCMS evaluates the original B2A pipelines independently. We bake the
//! modified samples into an identity-curve mBA CLUT so convex interpolation
//! cannot exceed the verified device limits. A2B remains the printer model.
use super::{Profile,R,Lut,make_mba,put16};
use lcms2::{Tag,TagSignature};
#[derive(Debug,Clone,Copy)]
pub struct Selective{
 pub center:f64,pub width:f64,pub light:f64,pub chroma_percent:f64,pub hue_shift:f64,
}
impl Default for Selective{fn default()->Self{Self{center:135.,width:45.,light:0.,chroma_percent:0.,hue_shift:0.}}}
impl Selective{
 fn check(self)->R<()>{
  if ![self.center,self.width,self.light,self.chroma_percent,self.hue_shift].iter().all(|v|v.is_finite())
   || !(0.0..=360.).contains(&self.center)||!(10.0..=180.).contains(&self.width)
   || !(-30.0..=30.).contains(&self.light)||!(-80.0..=80.).contains(&self.chroma_percent)||!(-60.0..=60.).contains(&self.hue_shift){return Err("Invalid selective color parameters".into())}
  Ok(())
 }
}
fn smoothstep(v:f64)->f64{let v=v.clamp(0.,1.);v*v*(3.-2.*v)}
/// D50 Lab to LCh, soft hue mask, protect neutrals and L*=0/100.
pub fn corrected_lab(lab:[f64;3],c:Selective)->[f64;3]{
 let [l,a,b]=lab;let chroma=a.hypot(b);
 let hue=b.atan2(a).to_degrees().rem_euclid(360.);
 let distance=((hue-c.center+180.).rem_euclid(360.)-180.).abs();
 let weight=smoothstep(1.-distance/c.width)*smoothstep((chroma-3.)/12.)*smoothstep(l/8.)*smoothstep((100.-l)/8.);
 if weight==0.|| (c.light==0.&&c.chroma_percent==0.&&c.hue_shift==0.){return lab}
 let h=(hue+c.hue_shift*weight).to_radians();
 let ch=chroma*(1.+c.chroma_percent/100.*weight);
 [(l+c.light*weight).clamp(0.,100.),(ch*h.cos()).clamp(-128.,127.),(ch*h.sin()).clamp(-128.,127.)]
}
fn lab_at(i:usize,j:usize,k:usize,g:usize)->[f64;3]{
 let d=(g-1) as f64;[100.*i as f64/d,255.*j as f64/d-128.,255.*k as f64/d-128.]
}
fn normalized(lab:[f64;3])->[f32;3]{[(lab[0]/100.) as f32,((lab[1]+128.)/255.) as f32,((lab[2]+128.)/255.) as f32]}
fn constrained(mut v:[f64;4],caps:[f64;4],tac:f64)->R<[f64;4]>{
 if v.iter().any(|v|!v.is_finite()){return Err("Non-finite LittleCMS pipeline output".into())}
 for i in 0..4{v[i]=v[i].clamp(0.,caps[i]);}
 let sum=v.iter().sum::<f64>();if sum>tac{for x in &mut v{*x*=tac/sum;}}
 Ok(v)
}
fn smooth_nodes(nodes:&mut Vec<[f64;4]>,g:usize,strength:f64,passes:usize){
 let plane=g*g;
 for _ in 0..passes{
  let mut next=nodes.clone();
  // Leave lattice boundary unchanged. Interior averaging uses nonnegative
  // weights summing to one, preserving caps and total ink of every node.
  for l in 1..g-1{for a in 1..g-1{for b in 1..g-1{
   let i=l*plane+a*g+b;
   let neighbors=[i-plane,i+plane,i-g,i+g,i-1,i+1];
   for j in 0..4{next[i][j]=nodes[i][j]*(1.-strength)+strength*neighbors.iter().map(|&n|nodes[n][j]).sum::<f64>()/6.;}
  }}}
  *nodes=next;
 }
}
#[derive(Clone,Copy,Debug)]
pub enum Operation{Selective(Selective),Smooth{strength:f64,passes:usize}}
/// Explicit total ink cap is a user setting, not inferred physical calibration.
/// Source per-channel upper bounds are independently obtained from LUT curves.
impl Profile{
 pub fn bounded_correction(&self,op:Operation,grid:usize,tac_percent:f64)->R<Self>{
  self.check()?;
  if !self.cmyk()||self.version_major()!=4||self.pcs()!="Lab "{return Err("Correction preview requires ICC v4 CMYK / Lab printer profile".into())}
  if ![17,33,49,65].contains(&grid)||!tac_percent.is_finite()||!(0.0..=400.).contains(&tac_percent){return Err("Invalid correction grid or TAC".into())}
  match op{
   Operation::Selective(c)=>{c.check()?;if c.light==0.&&c.chroma_percent==0.&&c.hue_shift==0.{return Err("Choose a nonzero color adjustment first".into())}},
   Operation::Smooth{strength,passes}=>{if !strength.is_finite()||!(0.05..=0.5).contains(&strength)||!(1..=5).contains(&passes){return Err("Smoothing strength 5–50%, passes 1–5".into())}}
  }
  let tac=tac_percent/100.;
  let global_caps=self.verify([1.;4]).map_err(|e|format!("Cannot establish source limits: {e}"))?;
  let native=lcms2::Profile::new_icc(&self.bytes()?).map_err(|e|e.to_string())?;
  let mut out=self.clone();
  for (key,sig) in [(b"B2A0",TagSignature::BToA0Tag),(b"B2A1",TagSignature::BToA1Tag),(b"B2A2",TagSignature::BToA2Tag)]{
   let raw=self.tags.get(key).ok_or("All three B2A intents are required")?;
   if !raw.starts_with(b"mBA "){return Err("Correction requires v4 mBA LUTs".into())}
   // Validate supported simple structure, dimensions and source bounds.
   let old=Lut::parse(raw)?;
   if old.inputs!=3||old.outputs!=4{return Err("Unexpected B2A dimensions".into())}
   let Tag::Pipeline(pipeline)=native.read_tag(sig)else{return Err("LittleCMS could not decode B2A pipeline".into())};
   if pipeline.input_channels()!=3||pipeline.output_channels()!=4{return Err("Unexpected LittleCMS pipeline dimensions".into())}
   let mut nodes=Vec::with_capacity(grid.pow(3));
   for l in 0..grid{for a in 0..grid{for b in 0..grid{
    let lab=lab_at(l,a,b,grid);
    let lab=match op{Operation::Selective(c)=>corrected_lab(lab,c),Operation::Smooth{..}=>lab};
    let mut result=[0f32;4];
    pipeline.eval(&normalized(lab),&mut result);
    nodes.push(constrained(result.map(|v|v as f64),global_caps,tac)?);
   }}}
   if let Operation::Smooth{strength,passes}=op{smooth_nodes(&mut nodes,grid,strength,passes);}
   let mut table=Vec::with_capacity(nodes.len()*8);
   for node in nodes{
    let node=constrained(node,global_caps,tac)?;
    for v in node{put16(&mut table,(v*65535.).floor() as u16);}
   }
   out.tags.insert(*key,make_mba(grid,&table));
  }
  out.clean_stale();
  out.verify(global_caps)?;
  // Integer CLUT values, identity output curves: convex interpolation
  // guarantees TAC <= requested value at all positions, not only samples.
  for key in [b"B2A0",b"B2A1",b"B2A2"]{
   let lut=Lut::parse(out.tags.get(key).unwrap())?;
   if lut.clut.chunks_exact(4).any(|n|n.iter().sum::<f64>()>tac+1e-10){return Err("Generated LUT exceeds total ink cap".into())}
  }
  Ok(out)
 }
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn neutral_and_white_locked(){let c=Selective{light:10.,chroma_percent:40.,hue_shift:30.,..Default::default()};for p in [[50.,0.,0.],[100.,-30.,30.],[0.,-30.,30.]]{assert_eq!(corrected_lab(p,c),p);}}
 #[test]fn hue_wrap_is_continuous(){let c=Selective{center:0.,width:30.,light:10.,..Default::default()};let a=corrected_lab([50.,40.,0.01],c);let b=corrected_lab([50.,40.,-0.01],c);assert!((a[0]-b[0]).abs()<1e-9);assert!(a[0]>59.);}
 #[test]fn selective_leaves_other_hues(){let c=Selective{center:135.,width:25.,light:10.,..Default::default()};let p=[50.,40.,0.];assert_eq!(corrected_lab(p,c),p);}
 #[test]fn zero_k_and_tac_preserved(){let mut n=vec![[0.1,0.1,0.1,0.];27];n[13]=constrained([1.,1.,1.,1.],[1.,1.,1.,0.],2.4).unwrap();let boundary=n[0];smooth_nodes(&mut n,3,0.5,3);assert_eq!(n[0],boundary);assert!(n[13][0]<0.8);for p in n{assert_eq!(p[3],0.);assert!(p.iter().sum::<f64>()<=2.4+1e-12)}}
 #[test]fn bounds_validation(){assert!(constrained([f64::NAN,0.,0.,0.],[1.;4],3.).is_err());assert_eq!(constrained([1.;4],[1.,1.,1.,0.],0.).unwrap(),[0.;4]);}
}
