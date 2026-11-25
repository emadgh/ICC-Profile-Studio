//! Selective LCh correction and bounded separation smoothing.
//! LittleCMS evaluates the original B2A pipelines independently. We bake the
//! modified samples into an identity-curve mBA CLUT so convex interpolation
//! cannot exceed the verified device limits. A2B remains the printer model.
use super::{Profile,R,Lut,make_mba,put16};
use lcms2::{Tag,TagSignature,CIELab,CIELabExt};
#[derive(Debug,Clone,Copy,serde::Serialize,serde::Deserialize)]
pub struct Selective{
 pub center:f64,pub width:f64,pub light:f64,pub chroma_percent:f64,pub hue_shift:f64,
}
impl Default for Selective{fn default()->Self{Self{center:135.,width:45.,light:0.,chroma_percent:0.,hue_shift:0.}}}
impl Selective{
 pub(crate) fn check(self)->R<()>{
  if ![self.center,self.width,self.light,self.chroma_percent,self.hue_shift].iter().all(|v|v.is_finite())
   || !(0.0..=360.).contains(&self.center)||!(10.0..=180.).contains(&self.width)
   || !(-30.0..=30.).contains(&self.light)||!(-80.0..=80.).contains(&self.chroma_percent)||!(-60.0..=60.).contains(&self.hue_shift){return Err("Invalid selective color parameters".into())}
  Ok(())
 }
}
fn smoothstep(v:f64)->f64{let v=v.clamp(0.,1.);v*v*(3.-2.*v)}
pub const MAX_REGIONS:usize=6;
impl Selective{
 pub fn active(self)->bool{self.light!=0.||self.chroma_percent!=0.||self.hue_shift!=0.}
 fn weight(self,l:f64,chroma:f64,hue:f64)->f64{
  let distance=((hue-self.center+180.).rem_euclid(360.)-180.).abs();
  smoothstep(1.-distance/self.width)*smoothstep((chroma-3.)/12.)*smoothstep(l/8.)*smoothstep((100.-l)/8.)
 }
}
/// All active masks are evaluated against the original PCS color. Normalize
/// overlapping contributions only when their sum exceeds one: no stacking
/// beyond the largest requested adjustment. Inactive masks do not dilute
/// active ones. Outside the masks, return the input bit-for-bit.
pub fn corrected_regions(lab:[f64;3],regions:&[Selective])->[f64;3]{
 let [l,a,b]=lab;let chroma=a.hypot(b);let hue=b.atan2(a).to_degrees().rem_euclid(360.);
 let (mut total,mut dl,mut dc,mut dh)=(0.,0.,0.,0.);
 for &r in regions{if !r.active(){continue}let w=r.weight(l,chroma,hue);total+=w;dl+=r.light*w;dc+=r.chroma_percent*w;dh+=r.hue_shift*w;}
 if total==0.||(dl==0.&&dc==0.&&dh==0.){return lab}
 let norm=total.max(1.);let h=(hue+dh/norm).to_radians();let ch=chroma*(1.+dc/(100.*norm));
 [(l+dl/norm).clamp(0.,100.),(ch*h.cos()).clamp(-128.,127.),(ch*h.sin()).clamp(-128.,127.)]
}
pub fn corrected_lab(lab:[f64;3],c:Selective)->[f64;3]{corrected_regions(lab,&[c])}
fn validate_regions(regions:&[Selective])->R<()>{
 if regions.is_empty()||regions.len()>MAX_REGIONS{return Err("Choose 1–6 color regions".into())}
 for &r in regions{r.check()?;}
 if !regions.iter().any(|r|r.active()){return Err("Choose a nonzero color adjustment first".into())}
 Ok(())
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
#[derive(Clone,Copy,Debug,serde::Serialize,serde::Deserialize)]
pub struct BlackGeneration{
 pub start:f64,pub maximum:f64,pub gamma:f64,pub strength:f64,pub tolerance:f64,pub ucr:bool,
}
impl Default for BlackGeneration{fn default()->Self{Self{start:0.20,maximum:1.,gamma:1.,strength:0.75,tolerance:2.,ucr:false}}}
impl BlackGeneration{
 pub(crate) fn check(self)->R<()>{
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
pub(crate) fn validate_points(points:[[f64;5];4])->R<()>{
 for p in points{
  if p[0]!=0.||p[4]!=1.||p.iter().any(|v|!v.is_finite()||!(0.0..=1.).contains(v))||p.windows(2).any(|v|v[0]>v[1]){return Err("Curves must be monotone, with fixed 0% and 100% endpoints".into())}
 }
 Ok(())
}
fn curve_points(v:[f64;4],points:[[f64;5];4])->[f64;4]{
 std::array::from_fn(|i|super::interp(&points[i],v[i]))
}
#[derive(Clone,Copy,Debug)]
pub enum Operation{Selective(Selective),Regions{regions:[Selective;MAX_REGIONS],count:usize},Smooth{strength:f64,passes:usize},Optimize{weights:[f64;4],tolerance:f64},Black(BlackGeneration),Points([[f64;5];4])}
/// Explicit total ink cap is a user setting, not inferred physical calibration.
/// Source per-channel upper bounds are independently obtained from LUT curves.
impl Profile{
 pub fn bounded_correction(&self,op:Operation,grid:usize,tac_percent:f64)->R<Self>{
  self.check()?;
  if !self.cmyk()||self.version_major()!=4||self.pcs()!="Lab "{return Err("Correction preview requires ICC v4 CMYK / Lab printer profile".into())}
  if ![17,33,49,65].contains(&grid)||!tac_percent.is_finite()||!(0.0..=400.).contains(&tac_percent){return Err("Invalid correction grid or TAC".into())}
  match op{
   Operation::Selective(c)=>{c.check()?;if c.light==0.&&c.chroma_percent==0.&&c.hue_shift==0.{return Err("Choose a nonzero color adjustment first".into())}},
   Operation::Regions{regions,count}=>{if count>MAX_REGIONS{return Err("Too many correction regions".into())}validate_regions(&regions[..count])?;},
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
    let lab=match op{Operation::Selective(c)=>corrected_lab(lab,c),Operation::Regions{regions,count}=>corrected_regions(lab,&regions[..count]),_=>lab};
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
 #[test]fn regions_overlap_is_bounded_and_order_independent(){
 let a=Selective{center:0.,width:45.,light:10.,chroma_percent:20.,hue_shift:15.};
 let b=Selective{light:20.,chroma_percent:-10.,hue_shift:-5.,..a};
 let input=[50.,40.,0.];let x=corrected_regions(input,&[a,b]);let y=corrected_regions(input,&[b,a]);
 for i in 0..3{assert!((x[i]-y[i]).abs()<1e-10);}
 assert!((x[0]-65.).abs()<1e-10);
 assert_eq!(corrected_regions(input,&[a,Selective{light:0.,chroma_percent:0.,hue_shift:0.,..a}]),corrected_lab(input,a));
 assert_eq!(corrected_regions([50.,-40.,0.],&[a,b]),[50.,-40.,0.]);
 assert_eq!(corrected_regions([50.,0.,0.],&[a,b]),[50.,0.,0.]);
 }
 #[test]fn regions_disjoint_and_invalid(){
 let a=Selective{center:0.,width:20.,light:6.,..Default::default()};
 let b=Selective{center:180.,light:-5.,..a};
 assert_eq!(corrected_regions([50.,40.,0.],&[a,b]),corrected_lab([50.,40.,0.],a));
 assert_eq!(corrected_regions([50.,-40.,0.],&[a,b]),corrected_lab([50.,-40.,0.],b));
 assert!(validate_regions(&[]).is_err());assert!(validate_regions(&[Selective::default()]).is_err());
 assert!(validate_regions(&[Selective{width:f64::NAN,..a}]).is_err());
 }
 #[test]fn native_multi_region_preserves_forward_and_k0(){
 let p=crate::synthetic_cmyk_v4(true);
 let mut regions=[Selective::default();MAX_REGIONS];
 regions[0]=Selective{light:4.,center:40.,..Default::default()};
 regions[1]=Selective{chroma_percent:15.,center:270.,..Default::default()};
 let out=p.bounded_correction(Operation::Regions{regions,count:2},17,100.).unwrap();
 assert_eq!(out.verify([1.,1.,1.,0.]).unwrap()[3],0.);
 for tag in [b"A2B0",b"A2B1",b"A2B2"]{assert_eq!(p.tags.get(tag),out.tags.get(tag));}
 let dir=tempfile::tempdir().unwrap();let path=dir.path().join("multi.icc");out.save_new(&path).unwrap();
 assert!(crate::inspection::inspect(&path).unwrap().intents.iter().all(|i|i.result.as_ref().is_ok_and(|v|v[3]==0.)));
 assert!(p.bounded_correction(Operation::Regions{regions,count:MAX_REGIONS+1},17,100.).is_err());
 }
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

#[derive(Clone,Debug)]
pub enum DynamicCorrection{Points(Vec<[f64;5]>),Regions{regions:[Selective;MAX_REGIONS],count:usize},Smooth{strength:f64,passes:usize},Optimize{weights:Vec<f64>,tolerance:f64,caps:Vec<f64>},Black{channel:usize,settings:BlackGeneration,caps:Vec<f64>}}
// Flat interleaved storage. Each pass is a Jacobi update from the previous
// lattice, with a convex combination of the six axial neighbors.
fn smooth_dynamic(values:&mut Vec<f64>,g:usize,n:usize,strength:f64,passes:usize){
 let plane=g*g;
 let mut next=values.clone();
 for _ in 0..passes{
  for l in 1..g-1{for a in 1..g-1{for b in 1..g-1{
   let i=l*plane+a*g+b;
   let neighbors=[i-plane,i+plane,i-g,i+g,i-1,i+1];
   for j in 0..n{
    next[i*n+j]=(1.-strength)*values[i*n+j]+strength*neighbors.iter().map(|&k|values[k*n+j]).sum::<f64>()/6.;
   }
  }}}
  std::mem::swap(values,&mut next);
 }
}
// Bounded cost descent per LUT node. A candidate may only replace an
// already feasible separation if its quantized cost decreases and the
// appearance remains within tolerance of the original, unmodified node.
fn optimize_dynamic_node(base:&[f64],caps:&[f64],tac:f64,weights:&[f64],tol:f64,mut predict:impl FnMut(&[f64])->R<CIELab>)->R<Vec<f64>>{
 let n=base.len();let target=predict(base)?;
 let mut best:Vec<f64>=base.iter().map(|&v|(v.clamp(0.,1.)*65535.+1e-8).floor()/65535.).collect();
 if best.iter().zip(caps).any(|(&x,&c)|x>c+1e-10)||best.iter().sum::<f64>()>tac+1e-10{return Err("Source separation exceeds requested limits; rebuild before ink optimization".into())}
 let de=predict(&best)?.cie2000_delta_e(&target,1.,1.,1.);
 if !de.is_finite()||de>tol{return Err("Baseline quantization exceeds color tolerance".into())}
 let cost=|v:&[f64]|v.iter().zip(weights).map(|(x,w)|x*w).sum::<f64>();
 let mut best_cost=cost(&best);
 // Sort once by relative cost; consider only three cheapest active
 // replacements per channel, plus direct reduction. Bounded search,
 // not a globally optimal solution. At most 8*n trials per step.
 let mut order:Vec<usize>=(0..n).filter(|&i|caps[i]>0.).collect();order.sort_by(|&i,&j|weights[i].total_cmp(&weights[j]));
 let mut trial=best.clone();
 for step in [0.05,0.02,0.01]{
  for _ in 0..2{
   let mut changed=false;
   for &i in order.iter().rev(){
    if best[i]+1e-12<step{continue}
    for dest in std::iter::once(None).chain(order.iter().copied().filter(|&j|j!=i&&weights[j]<weights[i]).take(3).map(Some)){
     trial.copy_from_slice(&best);trial[i]-=step;
     if let Some(j)=dest{trial[j]+=step;}
     if trial.iter().zip(caps).any(|(&v,&c)|v<0.||v>c+1e-12)||trial.iter().sum::<f64>()>tac+1e-10{continue}
     for v in &mut trial{*v=(*v*65535.+1e-8).floor()/65535.;}
     let c=cost(&trial);if c+1e-9>=best_cost{continue}
     let de=predict(&trial)?.cie2000_delta_e(&target,1.,1.,1.);
     if !de.is_finite(){return Err("Invalid optimization Delta E".into())}
     if de<=tol{best.copy_from_slice(&trial);best_cost=c;changed=true;}
    }
   }
   if !changed{break}
  }
 }
 Ok(best)
}
// User identifies the physical black ink. No channel-name heuristic and
// no assumption that any three other inks are CMY. Search uses bounded
// one-ink and distributed compensations against the native forward model.
fn black_dynamic_node(base:&[f64],caps:&[f64],tac:f64,k:usize,s:BlackGeneration,mut predict:impl FnMut(&[f64])->R<CIELab>)->R<Vec<f64>>{
 let n=base.len();let target=predict(base)?;
 if ![target.L,target.a,target.b].iter().all(|v|v.is_finite()){return Err("Invalid baseline Lab".into())}
 let mut best:Vec<f64>=base.iter().map(|&v|(v.clamp(0.,1.)*65535.+1e-8).floor()/65535.).collect();
 if best.iter().zip(caps).any(|(&x,&c)|x>c+1e-10)||best.iter().sum::<f64>()>tac+1e-10{return Err("Source separation exceeds requested limits; rebuild before black generation".into())}
 let de=predict(&best)?.cie2000_delta_e(&target,1.,1.,1.);
 if !de.is_finite()||de>s.tolerance{return Err("Baseline quantization exceeds color tolerance".into())}
 let desired=s.target(target,base[k],caps[k]);
 let mut error=(best[k]-desired).abs();
 let mut trial=best.clone();
 let active:Vec<usize>=(0..n).filter(|&i|i!=k&&caps[i]>0.).collect();
 // A candidate must improve black preference and satisfy the same
 // baseline color constraint, never a moving color target.
 for step in [0.10,0.05,0.02,0.01,0.005]{
  for _ in 0..2{
   let delta=(desired-best[k]).clamp(-step,step);
   if delta.abs()<1./65535.{break}
   let mut accepted=false;
   for mode in 0..(1+active.len()+3){
    trial.copy_from_slice(&best);trial[k]+=delta;
    if mode>0&&mode<=active.len(){trial[active[mode-1]]-=delta;}
    else if mode>active.len()&&!active.is_empty(){
     // Distribute replacement over currently available non-black ink.
     let ratio=[0.5,1.,2.][mode-active.len()-1];
     let room=|i:usize|if delta>0.{best[i]}else{(caps[i]-best[i]).max(0.)};
     let total=active.iter().map(|&i|room(i)).sum::<f64>();
     if total<=0.{continue}
     for &i in &active{trial[i]-=delta*ratio*room(i)/total;}
    }
    if trial.iter().zip(caps).any(|(&v,&c)|v<0.||v>c+1e-12)||trial.iter().sum::<f64>()>tac+1e-10{continue}
    for v in &mut trial{*v=(*v*65535.+1e-8).floor()/65535.;}
    let e=(trial[k]-desired).abs();if e+1e-9>=error{continue}
    let de=predict(&trial)?.cie2000_delta_e(&target,1.,1.,1.);
    if !de.is_finite(){return Err("Invalid black-generation Delta E".into())}
    if de<=s.tolerance{best.copy_from_slice(&trial);error=e;accepted=true;break}
   }
   if !accepted{break}
  }
 }
 Ok(best)
}
// Raw LUT16 PCS uses 0xff00 for nominal ICC v2 Lab endpoints.
const LEGACY_LAB:f64=65280./65535.;
fn correction_input(raw:[f64;3],legacy:bool,regions:&[Selective])->[f32;3]{
 let factor=if legacy{LEGACY_LAB}else{1.};
 if regions.is_empty()||raw.iter().any(|&x|x>factor){return raw.map(|v|v as f32)}
 let lab=[raw[0]/factor*100.,raw[1]/factor*255.-128.,raw[2]/factor*255.-128.];
 let c=corrected_regions(lab,regions);
 [c[0]/100.,(c[1]+128.)/255.,(c[2]+128.)/255.].map(|v|(v.clamp(0.,1.)*factor) as f32)
}
pub(crate) fn correction_kind(p:&Profile)->R<&'static [u8;4]>{
 crate::multichannel::channel_count(p)?;
 let kind=if p.version_major()==2{b"mft2"}else{b"mBA "};
 if p.version_major()==2&&!p.tags.get(b"A2B1").is_some_and(|v|v.starts_with(b"mft2")){return Err("ICC v2 correction requires LUT16 (mft2) A2B1".into())}
 for k in [b"B2A0",b"B2A1",b"B2A2"]{if !p.tags.get(k).is_some_and(|v|v.starts_with(kind)){return Err("Correction requires all three reverse LUTs: v2 mft2 or v4 mBA".into())}}
 Ok(kind)
}
impl Profile{
 /// CMYK/nCLR v2 LUT16 or v4 Lab correction, with independent reverse intents and certified ink limits.
 /// Black generation requires an explicit physical channel index. A2B stays byte-identical.
 pub fn bounded_correction_multi(&self,op:DynamicCorrection,grid:usize,tac_percent:f64)->R<Self>{
  let n=crate::multichannel::channel_count(self)?;
  let kind=correction_kind(self)?;let legacy=self.version_major()==2;
  if !(if self.cmyk(){[17,33,49,65].contains(&grid)}else{[17,33].contains(&grid)})||!tac_percent.is_finite()||!(0.0..=n as f64*100.).contains(&tac_percent){return Err("Requires supported grid and valid TAC".into())}
  if legacy&&matches!(&op,DynamicCorrection::Optimize{..}|DynamicCorrection::Black{..}){return Err("Whole-profile optimization and black generation require ICC v4 Lab".into())}
  match &op{
   DynamicCorrection::Points(p)=>{if p.len()!=n{return Err("Point curve count must match channels".into())}for &row in p{validate_points([row;4])?;}},
   DynamicCorrection::Regions{regions,count}=>{if *count>MAX_REGIONS{return Err("Too many correction regions".into())}validate_regions(&regions[..*count])?;},
   DynamicCorrection::Smooth{strength,passes}=>{if !strength.is_finite()||!(0.05..=0.5).contains(strength)||!(1..=5).contains(passes){return Err("Smoothing strength 5–50%, passes 1–5".into())}},
   DynamicCorrection::Optimize{weights,tolerance,caps}=>{crate::optimizer::MultiSettings{caps:caps.clone(),tac:tac_percent,weights:weights.clone(),tolerance:*tolerance}.validate(n)?;},
   DynamicCorrection::Black{channel,settings,caps}=>{settings.check()?;if *channel>=n||caps.len()!=n||caps.iter().any(|v|!v.is_finite()||!(0.0..=100.).contains(v)){return Err("Invalid physical black channel or ink caps".into())}}
  }
  let mut caps=crate::print_space::bounds(self)?.channels;
  if let DynamicCorrection::Optimize{caps:requested,..}|DynamicCorrection::Black{caps:requested,..}=&op{for (x,&c) in caps.iter_mut().zip(requested){*x=x.min(c/100.);}}
  if let DynamicCorrection::Black{channel,..}=&op{if caps[*channel]==0.{return Err("Selected physical black channel is closed; rebuild or select an active physical black ink".into())}}
  let tac=tac_percent/100.;
  let native=lcms2::Profile::new_icc(&self.bytes()?).map_err(|e|e.to_string())?;
  let forward=if matches!(&op,DynamicCorrection::Optimize{..}|DynamicCorrection::Black{..}){let Tag::Pipeline(p)=native.read_tag(TagSignature::AToB1Tag)else{return Err("Ink optimization requires A2B1".into())};if p.input_channels() as usize!=n||p.output_channels()!=3{return Err("Unexpected A2B1 dimensions".into())}Some(p)}else{None};
  let mut out=self.clone();
  let mut input=vec![0f32;n];
  for (key,sig) in [(b"B2A0",TagSignature::BToA0Tag),(b"B2A1",TagSignature::BToA1Tag),(b"B2A2",TagSignature::BToA2Tag)]{
   // Encoding checked by correction_kind.
   // The source reverse envelope was certified by print_space::bounds; native evaluation supports complex PCS stages.
   let Tag::Pipeline(pipe)=native.read_tag(sig)else{return Err("LittleCMS could not decode B2A".into())};
   if pipe.input_channels()!=3||pipe.output_channels() as usize!=n{return Err("Unexpected native pipeline dimensions".into())}
   let mut result=vec![0f32;n];let mut values=Vec::with_capacity(grid.pow(3)*n);let mut node=vec![0.;n];
   for l in 0..grid{for a in 0..grid{for b in 0..grid{
    let den=(grid-1) as f64;let raw=[l as f64/den,a as f64/den,b as f64/den];
    let regions=match &op{DynamicCorrection::Regions{regions,count}=>&regions[..*count],_=>&[]};
    pipe.eval(&correction_input(raw,legacy,regions),&mut result);
    if result.iter().any(|v|!v.is_finite()||*v< -0.00002||*v>1.00002){return Err("Invalid native source separation".into())}
    if let DynamicCorrection::Black{channel,settings,..}=&op{
     let p=forward.as_ref().ok_or("Missing A2B1")?;
     for (x,&v) in node.iter_mut().zip(&result){*x=f64::from(v).clamp(0.,1.);}
     let best=black_dynamic_node(&node,&caps,tac,*channel,*settings,|v|{
      for (x,&y) in input.iter_mut().zip(v){*x=y as f32;}let mut lab=[0f32;3];p.eval(&input,&mut lab);
      if lab.iter().any(|v|!v.is_finite()){return Err("Invalid forward Lab".into())}
      Ok(CIELab{L:100.*lab[0] as f64,a:255.*lab[1] as f64-128.,b:255.*lab[2] as f64-128.})
     })?;
     values.extend_from_slice(&best);continue
    }
    if let DynamicCorrection::Optimize{weights,tolerance,..}=&op{
     let p=forward.as_ref().ok_or("Missing A2B1")?;
     for (x,&v) in node.iter_mut().zip(&result){*x=f64::from(v).clamp(0.,1.);}
     let best=optimize_dynamic_node(&node,&caps,tac,weights,*tolerance,|v|{
      for (x,&y) in input.iter_mut().zip(v){*x=y as f32;}let mut lab=[0f32;3];p.eval(&input,&mut lab);
      if lab.iter().any(|v|!v.is_finite()){return Err("Invalid forward Lab".into())}
      Ok(CIELab{L:100.*lab[0] as f64,a:255.*lab[1] as f64-128.,b:255.*lab[2] as f64-128.})
     })?;
     values.extend_from_slice(&best);continue
    }
    for (j,&v) in result.iter().enumerate(){
     let v=f64::from(v).clamp(0.,1.);
     let v=match &op{DynamicCorrection::Points(p)=>super::interp(&p[j],v),_=>v};
     node[j]=v.clamp(0.,caps[j]);
    }
    let sum=node.iter().sum::<f64>();if sum>tac&&sum>0.{for v in &mut node{*v*=tac/sum;}}
    values.extend_from_slice(&node);
   }}}
   if let DynamicCorrection::Smooth{strength,passes}=&op{smooth_dynamic(&mut values,grid,n,*strength,*passes);}
   // Reproject after smoothing to guard floating-point rounding; floor to 16-bit.
   for row in values.chunks_exact_mut(n){
    for (j,v) in row.iter_mut().enumerate(){*v=v.clamp(0.,caps[j]);}
    let sum=row.iter().sum::<f64>();if sum>tac&&sum>0.{for v in row.iter_mut(){*v*=tac/sum;}}
    for v in row{*v=(*v*65535.+1e-8).floor()/65535.;}
   }
   let lut=Lut{inputs:3,outputs:n,grid,in_curves:vec![vec![0.,1.];3],out_curves:vec![vec![0.,1.];n],clut:values};
   out.tags.insert(*key,crate::editing::encode(&lut,kind)?);
  }
  out.clean_stale();crate::multichannel::verify(&out,&caps,tac)?;Ok(out)
 }
}
#[cfg(test)]
mod dynamic_tests{
 use super::*;
 #[test]fn legacy_lab_coordinates_and_extended_range(){
  let raw=[0.5*LEGACY_LAB,0.6*LEGACY_LAB,0.4*LEGACY_LAB];
  let mut r=Selective::default();r.center=315.;r.light=10.;r.width=180.;
  let expected=corrected_regions([50.,25.,-26.],&[r]);
  let actual=correction_input(raw,true,&[r]);
  let want=[expected[0]/100.,(expected[1]+128.)/255.,(expected[2]+128.)/255.].map(|v|(v.clamp(0.,1.)*LEGACY_LAB) as f32);
  for j in 0..3{assert!((actual[j]-want[j]).abs()<1e-6);}
  assert_eq!(correction_input([1.,0.5,0.5],true,&[r]),[1.,0.5,0.5]);
  assert_eq!(correction_input(raw,true,&[]),raw.map(|v|v as f32));
 }
 #[test]#[ignore="Requires locally supplied FOGRA55 ICC"]
 fn fogra55_legacy_correction_real_profile(){
  let path=std::path::PathBuf::from(std::env::var_os("ICC_FOGRA55_PATH").expect("Set ICC_FOGRA55_PATH"));
  let original=std::fs::read(&path).unwrap();let p=Profile::read(&path).unwrap();
  assert_eq!(p.version_major(),2);assert_eq!(p.space(),"7CLR");
  let bounds=crate::print_space::bounds(&p).unwrap();
  let q=p.bounded_correction_multi(DynamicCorrection::Smooth{strength:0.2,passes:2},17,300.).unwrap();
  let d=tempfile::tempdir().unwrap();let dest=d.path().join("smoothed.icc");q.save_new(&dest).unwrap();let q=Profile::read(&dest).unwrap();
  let v=crate::multichannel::verify(&q,&bounds.channels,3.).unwrap();println!("FOGRA55 smoothing: {:?}",v);
  for k in [b"A2B0",b"A2B1",b"A2B2"]{assert_eq!(p.tags[k],q.tags[k]);}
  for intent in [lcms2::Intent::Perceptual,lcms2::Intent::RelativeColorimetric,lcms2::Intent::Saturation,lcms2::Intent::AbsoluteColorimetric]{
   let report=crate::comparison::compare(&path,&dest,17,intent).unwrap();println!("{intent:?}: Mean/P95/Max DE00 {:.4}/{:.4}/{:.4}, TAC {:.4}%",report.mean_de,report.p95_de,report.max_de,report.candidate.max_tac);
  }
  assert_eq!(std::fs::read(&path).unwrap(),original);
 }
 #[test]fn legacy_seven_channel_corrections_preserve_encoding_and_intents(){
  let p=crate::versioning::test_legacy_fixture(7);
  let before=crate::print_space::bounds(&p).unwrap();
  let mut points=vec![[0.,0.25,0.5,0.75,1.];7];points[0]=[0.,0.4,0.65,0.8,1.];
  let mut regions=[Selective::default();MAX_REGIONS];regions[0].light=5.;
  for op in [DynamicCorrection::Points(points),DynamicCorrection::Smooth{strength:0.2,passes:2},DynamicCorrection::Regions{regions,count:1}]{
   let q=p.bounded_correction_multi(op,17,60.).unwrap();
   assert_eq!(q.version_major(),2);assert_eq!(q.space(),"7CLR");
   for k in [b"A2B0",b"A2B1",b"A2B2"]{assert_eq!(p.tags[k],q.tags[k]);}
   for k in [b"B2A0",b"B2A1",b"B2A2"]{assert!(q.tags[k].starts_with(b"mft2"));}
   let v=crate::multichannel::verify(&q,&before.channels,0.6).unwrap();assert_eq!(v.channels[6],0.);
   let native=lcms2::Profile::new_icc(&q.bytes().unwrap()).unwrap();
   for intent in [lcms2::Intent::Perceptual,lcms2::Intent::RelativeColorimetric,lcms2::Intent::Saturation,lcms2::Intent::AbsoluteColorimetric]{assert_eq!(crate::print_space::sample_rgb(&native,&[[255,255,255],[0,0,0],[125,60,80]],intent).unwrap()[6],0.);}
  }
  assert!(p.bounded_correction_multi(DynamicCorrection::Black{channel:3,settings:BlackGeneration::default(),caps:vec![100.;7]},17,60.).is_err());
  let mut bad=p.clone();bad.tags.remove(b"B2A2");assert!(correction_kind(&bad).is_err());
 }
 #[test]fn nclr_points_preserve_forward_caps_and_tac(){
  let p=crate::print_space::fixture(6);
  let mut points=vec![[0.,0.25,0.5,0.75,1.];6];points[0]=[0.,0.6,0.8,0.9,1.];
  let q=p.bounded_correction_multi(DynamicCorrection::Points(points),17,40.).unwrap();
  assert_eq!(p.tags[b"A2B1"],q.tags[b"A2B1"]);
  let b=crate::print_space::bounds(&q).unwrap();assert_eq!(b.channels[5],0.);assert!(b.tac<=0.4+1e-10);
  assert!(p.bounded_correction_multi(DynamicCorrection::Points(vec![[0.,0.25,0.5,0.75,1.];4]),17,40.).is_err());
 }
 #[test]fn nclr_regions_preserve_limits(){
  let p=crate::print_space::fixture(6);
  let mut regions=[Selective::default();MAX_REGIONS];regions[0].light=10.;
  let q=p.bounded_correction_multi(DynamicCorrection::Regions{regions,count:1},17,50.).unwrap();
  let b=crate::print_space::bounds(&q).unwrap();assert_eq!(b.channels[5],0.);assert!(b.tac<=0.5+1e-10);
 }
}

#[cfg(test)]
mod dynamic_smoothing_tests{
 use super::*;
 #[test]fn impulse_smoothing_preserves_boundary_and_closed_channel(){
  let n=6;let g=3;let mut v=vec![0.;g*g*g*n];v[13*n]=0.8;v[0]=0.3;
  smooth_dynamic(&mut v,g,n,0.5,2);
  assert_eq!(v[0],0.3);assert!((v[13*n]-0.2).abs()<1e-12);
  assert!(v.chunks_exact(n).all(|x|x[5]==0.&&x.iter().sum::<f64>()<=0.8));
 }
 #[test]fn nclr_smoothing_keeps_native_caps_and_forward(){
  let p=crate::print_space::fixture(6);
  let q=p.bounded_correction_multi(DynamicCorrection::Smooth{strength:0.3,passes:3},17,35.).unwrap();
  for k in [b"A2B0",b"A2B1",b"A2B2"]{assert_eq!(p.tags[k],q.tags[k]);}
  let before=crate::print_space::bounds(&p).unwrap();let after=crate::print_space::bounds(&q).unwrap();
  assert_eq!(after.channels[5],0.);assert!(after.tac<=0.35+1e-10);
  assert!(after.channels.iter().zip(before.channels).all(|(&a,b)|a<=b+1e-10));
  assert!(p.bounded_correction_multi(DynamicCorrection::Smooth{strength:f64::NAN,passes:1},17,35.).is_err());
  assert!(p.bounded_correction_multi(DynamicCorrection::Smooth{strength:0.3,passes:0},17,35.).is_err());
 }
}

#[cfg(test)]
mod dynamic_optimizer_tests{
 use super::*;
 #[test]fn constant_color_prefers_lower_cost_and_keeps_zero_channel(){
  let base=[0.5,0.3,0.2,0.1,0.4,0.];let caps=[1.,1.,1.,1.,1.,0.];let weights=[1.,2.,1.,1.,1.,1.];
  let predict=|_:&[f64]|Ok(CIELab{L:50.,a:0.,b:0.});
  let v=optimize_dynamic_node(&base,&caps,1.5,&weights,1.,predict).unwrap();
  assert_eq!(v[5],0.);assert!(v.iter().sum::<f64>()<1.5);
  assert!(optimize_dynamic_node(&base,&caps,0.1,&weights,1.,predict).is_err());
 }
 #[test]fn nclr_optimization_preserves_bounds_and_forward(){
  let p=crate::print_space::fixture(6);
  let q=p.bounded_correction_multi(DynamicCorrection::Optimize{weights:vec![1.;6],tolerance:2.,caps:vec![100.;6]},17,600.).unwrap();
  assert_eq!(p.tags[b"A2B1"],q.tags[b"A2B1"]);
  let a=crate::print_space::bounds(&p).unwrap();let b=crate::print_space::bounds(&q).unwrap();
  assert_eq!(b.channels[5],0.);assert!(b.channels.iter().zip(a.channels).all(|(&x,y)|x<=y+1e-10));
 }
}

#[cfg(test)]
mod dynamic_black_tests{
 use super::*;
 #[test]fn selected_index_is_used_and_other_closed_ink_stays_zero(){
  let b=[0.2,0.1,0.0,0.2,0.1,0.];let caps=[1.,1.,1.,1.,1.,0.];
  let s=BlackGeneration{start:0.,maximum:1.,strength:1.,..Default::default()};
  let v=black_dynamic_node(&b,&caps,2.,2,s,|_|Ok(CIELab{L:20.,a:0.,b:0.})).unwrap();
  assert!(v[2]>0.);assert_eq!(v[5],0.);assert!(v.iter().sum::<f64>()<=2.);
 }
 #[test]fn excessive_color_change_is_rejected(){
  let b=[0.2,0.1,0.,0.2,0.1,0.];let caps=[1.,1.,1.,1.,1.,0.];
  let s=BlackGeneration{start:0.,maximum:1.,strength:1.,tolerance:0.1,..Default::default()};
  let v=black_dynamic_node(&b,&caps,2.,2,s,|x|Ok(CIELab{L:20.,a:x[2]*127.,b:0.})).unwrap();
  assert_eq!(v[2],0.);
 }
 #[test]fn ucr_leaves_chromatic_region_at_original_quantized_recipe(){
  let b=[0.2,0.1,0.3,0.2,0.1,0.];let caps=[1.,1.,1.,1.,1.,0.];
  let s=BlackGeneration{start:0.,strength:1.,ucr:true,..Default::default()};
  let v=black_dynamic_node(&b,&caps,2.,2,s,|_|Ok(CIELab{L:20.,a:60.,b:0.})).unwrap();
  assert!(v.iter().zip(b).all(|(&x,y)|(x-y).abs()<=1./65535.));
 }
 #[test]fn baseline_outside_user_tac_is_rejected(){
  let b=[0.2,0.1,0.3,0.2,0.1,0.];let caps=[1.,1.,1.,1.,1.,0.];
  assert!(black_dynamic_node(&b,&caps,0.1,2,BlackGeneration::default(),|_|Ok(CIELab{L:20.,a:0.,b:0.})).is_err());
 }
 #[test]fn profile_requires_explicit_valid_open_channel(){
  let p=crate::print_space::fixture(6);
  for channel in [5,6]{assert!(p.bounded_correction_multi(DynamicCorrection::Black{channel,settings:BlackGeneration::default(),caps:vec![100.;6]},17,600.).is_err());}
  let q=p.bounded_correction_multi(DynamicCorrection::Black{channel:1,settings:BlackGeneration::default(),caps:vec![100.;6]},17,600.).unwrap();
  for k in [b"A2B0",b"A2B1",b"A2B2"]{assert_eq!(p.tags[k],q.tags[k]);}
  assert_eq!(crate::print_space::bounds(&q).unwrap().channels[5],0.);
 }
}
