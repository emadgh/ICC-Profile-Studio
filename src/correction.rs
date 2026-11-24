//! Selective LCh correction and bounded separation smoothing.
//! LittleCMS evaluates the original B2A pipelines independently. We bake the
//! modified samples into an identity-curve mBA CLUT so convex interpolation
//! cannot exceed the verified device limits. A2B remains the printer model.
use super::{Profile,R,Lut,make_mba,put16};
use lcms2::{Tag,TagSignature,CIELab,CIELabExt};
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
fn quantized(v:[f64;4])->[f64;4]{v.map(|v|(v.clamp(0.,1.)*65535.).floor()/65535.)}
fn ink_cost(v:[f64;4],weights:[f64;4])->f64{v.iter().zip(weights).map(|(x,w)|x*w).sum()}
fn optimize_node(base:[f64;4],caps:[f64;4],tac:f64,weights:[f64;4],tol:f64,mut predict:impl FnMut([f64;4])->R<CIELab>)->R<[f64;4]>{
 let target=predict(base)?;
 let mut best=quantized(base);
 if base.iter().zip(caps).any(|(&v,c)|v>c+1e-6)||best.iter().sum::<f64>()>tac+1e-9{return Err("Existing separation exceeds selected limits; rebuild it first rather than silently clipping during ink optimization.".into())}
 if predict(best)?.cie2000_delta_e(&target,1.,1.,1.)>tol{return Err("Selected tolerance is below source quantization error".into())}
 let mut cost=ink_cost(best,weights);
 for step in [0.05,0.02,0.01]{
  for _ in 0..2{
   let prior=best;
   for i in 0..4{
    for j in 0..=4{
     if j==i{continue}let mut v=best;v[i]-=step;
     if j<4{v[j]+=step;}
     if v.iter().zip(caps).any(|(&x,c)|x<0.||x>c)||v.iter().sum::<f64>()>tac+1e-10{continue}
     let v=quantized(v);let c=ink_cost(v,weights);
     if c+1e-9>=cost{continue}
     let de=predict(v)?.cie2000_delta_e(&target,1.,1.,1.);
     if !de.is_finite(){return Err("Non-finite optimization error".into())}
     if de<=tol{best=v;cost=c;}
    }
   }
   if prior==best{break}
  }
 }
 Ok(best)
}
/// A predictive black-generation preference, not a measured separation
/// characterization. GCR uses broad chroma support; UCR is near-neutral/dark.
#[derive(Clone,Copy,Debug)]
pub struct BlackGeneration{
 pub start:f64,pub maximum:f64,pub gamma:f64,pub strength:f64,pub tolerance:f64,pub ucr:bool,
}
impl Default for BlackGeneration{fn default()->Self{Self{start:0.20,maximum:1.,gamma:1.,strength:0.75,tolerance:2.,ucr:false}}}
impl BlackGeneration{
 fn check(self)->R<()>{
  if ![self.start,self.maximum,self.gamma,self.strength,self.tolerance].iter().all(|v|v.is_finite())||
   !(0.0..=0.95).contains(&self.start)||!(0.0..=1.).contains(&self.maximum)||!(0.25..=4.).contains(&self.gamma)||
   !(0.0..=1.).contains(&self.strength)||!(0.1..=5.).contains(&self.tolerance){return Err("Invalid black generation settings".into())}Ok(())
 }
 fn target(self,appearance:CIELab,base:f64,cap:f64)->f64{
  let dark=(1.-appearance.L/100.).clamp(0.,1.);
  let ramp=((dark-self.start)/(1.-self.start)).clamp(0.,1.).powf(self.gamma);
  let chroma=appearance.a.hypot(appearance.b);
  let region=if self.ucr{smoothstep(1.-chroma/25.)*smoothstep((dark-0.40)/0.35)}else{0.25+0.75*smoothstep(1.-chroma/120.)};
  let w=self.strength*region;
  (base+(ramp*self.maximum.min(cap)-base)*w).clamp(0.,cap)
 }
}
fn black_node(base:[f64;4],caps:[f64;4],tac:f64,s:BlackGeneration,mut predict:impl FnMut([f64;4])->R<CIELab>)->R<[f64;4]>{
 let target=predict(base)?;
 if ![target.L,target.a,target.b].iter().all(|v|v.is_finite()){return Err("Invalid baseline Lab".into())}
 let desired=s.target(target,base[3],caps[3]);
 let mut best=quantized(base);
 if base.iter().zip(caps).any(|(&v,c)|v>c+1e-6)||best.iter().sum::<f64>()>tac+1e-9{return Err("Source exceeds chosen bounds; rebuild before black generation".into())}
 let baseline_error=predict(best)?.cie2000_delta_e(&target,1.,1.,1.);
 if !baseline_error.is_finite()||baseline_error>s.tolerance{return Err("Baseline quantization exceeds tolerance".into())}
 // Primary objective is closeness to the requested K preference; only
 // accept an improvement if predicted appearance remains in tolerance.
 let score=|v:[f64;4]|(v[3]-desired).abs();
 for step in [0.10,0.05,0.02,0.01]{
  for _ in 0..3{
   let prior=best;
   let mut candidates=Vec::new();
   for i in 0..4{for sign in [-1.,1.]{
    let mut v=best;v[i]+=sign*step;candidates.push(v);
   }}
   for i in 0..4{for j in 0..4{if i!=j{let mut v=best;v[i]-=step;v[j]+=step;candidates.push(v);}}}
   for sign in [-1.,1.]{for ratio in [0.25,0.5,1.]{
    let mut v=best;v[3]+=sign*step;for x in &mut v[..3]{*x-=sign*step*ratio;}candidates.push(v);
   }}
   for v in candidates{
    if v.iter().zip(caps).any(|(&x,c)|x<0.||x>c)||v.iter().sum::<f64>()>tac+1e-10{continue}
    let v=quantized(v);
    if score(v)+1e-7>=score(best){continue}
    let de=predict(v)?.cie2000_delta_e(&target,1.,1.,1.);
    if !de.is_finite(){return Err("Non-finite black generation error".into())}
    if de<=s.tolerance{best=v;}
   }
   if prior==best{break}
  }
 }
 Ok(best)
}
pub const IDENTITY_POINTS:[[f64;5];4]=[[0.,0.25,0.5,0.75,1.];4];
fn validate_points(points:[[f64;5];4])->R<()>{
 for p in points{
  if p[0]!=0.||p[4]!=1.||p.iter().any(|v|!v.is_finite()||!(0.0..=1.).contains(v))||p.windows(2).any(|v|v[0]>v[1]){return Err("Curves must be monotone, with fixed 0% and 100% endpoints".into())}
 }
 Ok(())
}
fn curve_points(v:[f64;4],points:[[f64;5];4])->[f64;4]{
 std::array::from_fn(|i|super::interp(&points[i],v[i]))
}
#[derive(Clone,Copy,Debug)]
pub enum Operation{Selective(Selective),Smooth{strength:f64,passes:usize},Optimize{weights:[f64;4],tolerance:f64},Black(BlackGeneration),Points([[f64;5];4])}
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
   Operation::Black(s)=>{s.check()?;if grid>33{return Err("Black generation grid must be 17 or 33".into())}},
   Operation::Points(p)=>validate_points(p)?,
   Operation::Optimize{weights,tolerance}=>{if grid>33||weights.iter().any(|v|!v.is_finite()||!(0.01..=100.).contains(v))||!tolerance.is_finite()||!(0.1..=5.).contains(&tolerance){return Err("Ink optimization: grid 17 or 33, tolerance 0.1–5, positive weights required".into())}}
  }
  let tac=tac_percent/100.;
  let global_caps=self.verify([1.;4]).map_err(|e|format!("Cannot establish source limits: {e}"))?;
  let native=lcms2::Profile::new_icc(&self.bytes()?).map_err(|e|e.to_string())?;
  let forward=if matches!(op,Operation::Optimize{..}|Operation::Black(_)){
   let Tag::Pipeline(p)=native.read_tag(TagSignature::AToB1Tag)else{return Err("Ink optimization requires A2B1".into())};
   if p.input_channels()!=4||p.output_channels()!=3{return Err("Unexpected forward pipeline dimensions".into())}Some(p)
  }else{None};
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
    let lab=match op{Operation::Selective(c)=>corrected_lab(lab,c),_=>lab};
    let mut result=[0f32;4];
    pipeline.eval(&normalized(lab),&mut result);
    let result=result.map(|v|v as f64);
    if matches!(op,Operation::Optimize{..}|Operation::Black(_)){
     let p=forward.as_ref().ok_or("Missing forward pipeline")?;
     let predict=|v:[f64;4]|->R<CIELab>{
      let mut lab=[0f32;3];p.eval(&v.map(|x|x as f32),&mut lab);
      if lab.iter().any(|v|!v.is_finite()){return Err("Invalid forward Lab".into())}
      Ok(CIELab{L:100.*lab[0] as f64,a:255.*lab[1] as f64-128.,b:255.*lab[2] as f64-128.})
     };
     if result.iter().any(|v|!v.is_finite()||*v<0.||*v>1.000001){return Err("Invalid source separation".into())}
     nodes.push(match op{Operation::Optimize{weights,tolerance}=>optimize_node(result,global_caps,tac,weights,tolerance,predict)?,Operation::Black(s)=>black_node(result,global_caps,tac,s,predict)?,_=>unreachable!()});
    }else{let v=if let Operation::Points(points)=op{curve_points(result,points)}else{result};nodes.push(constrained(v,global_caps,tac)?);}
   }}}
   if let Operation::Smooth{strength,passes}=op{smooth_nodes(&mut nodes,grid,strength,passes);}
   let mut table=Vec::with_capacity(nodes.len()*8);
   for node in nodes{
    let node=constrained(node,global_caps,tac)?;
    for v in node{put16(&mut table,if matches!(op,Operation::Optimize{..}){(v*65535.).round() as u16}else{(v*65535.).floor() as u16});}
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
 #[test]fn optimization_keeps_tolerance_and_cost(){
 let predict=|v:[f64;4]|Ok(CIELab{L:100.-20.*v.iter().sum::<f64>(),a:0.,b:0.});
 let base=[0.4,0.3,0.2,0.];let result=optimize_node(base,[1.,1.,1.,0.],2.4,[2.,1.,1.,1.],0.5,predict).unwrap();
 assert_eq!(result[3],0.);assert!(ink_cost(result,[2.,1.,1.,1.])<ink_cost(base,[2.,1.,1.,1.]));
 assert!(predict(base).unwrap().cie2000_delta_e(&predict(result).unwrap(),1.,1.,1.)<=0.5);
 }
 #[test]fn black_preference_respects_original_zero_k(){
 let predict=|v:[f64;4]|Ok(CIELab{L:100.-20.*v.iter().sum::<f64>(),a:0.,b:0.});
 let s=BlackGeneration{start:0.,strength:1.,..Default::default()};
 let a=black_node([0.4,0.3,0.2,0.],[1.,1.,1.,0.],2.4,s,predict).unwrap();
 assert_eq!(a[3],0.);
 let b=black_node([0.4,0.3,0.2,0.],[1.;4],2.4,s,predict).unwrap();
 assert!(b[3]>0.);
 assert!(predict(b).unwrap().cie2000_delta_e(&predict([0.4,0.3,0.2,0.]).unwrap(),1.,1.,1.)<=s.tolerance);
 }
 #[test]fn ucr_protects_saturated_colors(){let s=BlackGeneration{ucr:true,..Default::default()};assert_eq!(s.target(CIELab{L:30.,a:80.,b:20.},0.1,1.),0.1);}
 #[test]fn points_monotone(){let mut p=IDENTITY_POINTS;assert_eq!(curve_points([0.25,0.5,0.75,1.],p),[0.25,0.5,0.75,1.]);p[0][2]=0.1;assert!(validate_points(p).is_err());}
 #[test]fn native_pipeline_correction_roundtrip(){
 let p=crate::synthetic_cmyk_v4(true);
 let o=p.bounded_correction(Operation::Selective(Selective{light:3.,..Default::default()}),17,100.).unwrap();
 assert_eq!(o.verify([1.,1.,1.,0.]).unwrap()[3],0.);
 let dir=tempfile::tempdir().unwrap();let file=dir.path().join("edited.icc");o.save_new(&file).unwrap();
 let inspection=crate::inspection::inspect(&file).unwrap();
 assert_eq!(inspection.intents.len(),4);assert!(inspection.intents.iter().all(|i|i.result.as_ref().is_ok_and(|v|v[3]==0.)));
 }
 #[test]fn bounds_validation(){assert!(constrained([f64::NAN,0.,0.,0.],[1.;4],3.).is_err());assert_eq!(constrained([1.;4],[1.,1.,1.,0.],0.).unwrap(),[0.;4]);}
}
