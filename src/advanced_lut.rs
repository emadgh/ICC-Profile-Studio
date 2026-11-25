//! Lossless structural access to ICC v4 mAB/mBA with unequal CLUT axes,
//! sampled/gamma/parametric curves and PCS-side M/matrix stages. No curve
//! approximation is performed by the channel permutation path.
use crate::{Profile,R,u16at,u32at};
use std::ops::Range;
const LIMIT:usize=16_000_000;
#[derive(Clone,Debug)]
struct Curves{ranges:Vec<Range<usize>>,parametric:usize,gamma:usize}
#[derive(Clone,Debug)]
struct Layout{forward:bool,inputs:usize,outputs:usize,axes:Vec<usize>,precision:usize,clut:Range<usize>,a:Option<Curves>,b:Option<Curves>,m:Option<Curves>,matrix:Option<Range<usize>>}
#[derive(Clone,Debug)]
pub struct Structure{
 pub tag:String,pub kind:String,pub inputs:usize,pub outputs:usize,pub axes:Vec<usize>,pub precision_bits:usize,pub sampled_curves:usize,pub gamma_curves:usize,pub parametric_curves:usize,pub matrix:bool,pub m_curves:bool,pub structural_reorder:bool,pub strict_sampled_bounds:bool
}
fn region(t:&[u8],start:usize,len:usize)->R<Range<usize>>{
 let end=start.checked_add(len).ok_or("LUT region overflow")?;
 if start<32||start%4!=0||end>t.len(){return Err("Invalid v4 stage bounds or alignment".into())}Ok(start..end)
}
fn curves(t:&[u8],offset:usize,n:usize)->R<Option<Curves>>{
 if offset==0{return Ok(None)}
 let mut c=Curves{ranges:Vec::with_capacity(n),parametric:0,gamma:0};let mut o=offset;
 for _ in 0..n{
  let h=region(t,o,12)?;
  let len=match &t[h.start..h.start+4]{
   b"curv"=>{let n=u32at(t,o+8)? as usize;if n>65536{return Err("Oversized sampled curve".into())}if n==1{c.gamma+=1;}12+2*n},
   b"para"=>{let f=u16at(t,o+8)? as usize;let count=* [1usize,3,4,5,7].get(f).ok_or("Unknown ICC parametric curve function")?;c.parametric+=1;12+4*count},
   _=>return Err("Unknown ICC v4 curve type".into())
  };
  let aligned=(len+3)&!3;
  let r=region(t,o,aligned)?;o=r.end;c.ranges.push(r);
 }
 Ok(Some(c))
}
impl Layout{
 fn parse(t:&[u8])->R<Self>{
  if t.len()<32{return Err("Truncated v4 LUT".into())}
  let forward=match &t[..4]{b"mAB "=>true,b"mBA "=>false,_=>return Err("Expected mAB/mBA".into())};
  let (inputs,outputs)=(t[8] as usize,t[9] as usize);
  if !(1..=15).contains(&inputs)||!(1..=15).contains(&outputs){return Err("Invalid v4 LUT dimensions".into())}
  let pcs=if forward{outputs}else{inputs};let dev=if forward{inputs}else{outputs};
  if pcs!=3{return Err("Structural printer LUT support requires three PCS channels".into())}
  let bo=u32at(t,12)? as usize;let mo=u32at(t,16)? as usize;let co=u32at(t,20)? as usize;let cl=u32at(t,24)? as usize;let ao=u32at(t,28)? as usize;
  let b=curves(t,bo,pcs)?;let m=curves(t,co,pcs)?;let a=curves(t,ao,dev)?;
  let matrix=if mo==0{None}else{Some(region(t,mo,48)?)};
  if m.is_some()!=matrix.is_some(){return Err("Matrix and M curve stages must be paired".into())}
  // Only CLUT-based structures are handled, including optional device curves.
  // LittleCMS independently validates legal stage combinations.
  region(t,cl,20)?;
  let axes=t[cl..cl+inputs].iter().map(|&x|x as usize).collect::<Vec<_>>();
  if axes.iter().any(|&x|x<2){return Err("Invalid CLUT axis length".into())}
  let precision=t[cl+16] as usize;
  if ![1,2].contains(&precision){return Err("Invalid CLUT precision".into())}
  let count=axes.iter().try_fold(outputs,|v,&x|v.checked_mul(x)).filter(|&x|x<=LIMIT).ok_or("Oversized CLUT")?;
  let all=region(t,cl,20+count*precision)?;
  let clut=cl+20..all.end;
  let mut occupied=vec![all];
  for c in [&a,&b,&m].into_iter().flatten(){occupied.extend(c.ranges.iter().cloned());}
  if let Some(r)=&matrix{occupied.push(r.clone());}
  occupied.sort_by_key(|r|r.start);
  if occupied.windows(2).any(|r|r[0].end>r[1].start){return Err("Overlapping v4 LUT stages".into())}
  Ok(Self{forward,inputs,outputs,axes,precision,clut,a,b,m,matrix})
 }
 fn device_curves(&self)->Option<&Curves>{self.a.as_ref()}
}
fn describe(key:&[u8;4],t:&[u8])->R<Structure>{
 let l=Layout::parse(t)?;
 let mut sampled=0;let mut gamma=0;let mut parametric=0;
 for c in [&l.a,&l.b,&l.m].into_iter().flatten(){gamma+=c.gamma;parametric+=c.parametric;sampled+=c.ranges.len()-c.gamma-c.parametric;}
 let strict_sampled_bounds=!l.forward&&l.device_curves().is_none_or(|c|c.gamma==0&&c.parametric==0);
 Ok(Structure{tag:String::from_utf8_lossy(key).into_owned(),kind:String::from_utf8_lossy(&t[..4]).into_owned(),inputs:l.inputs,outputs:l.outputs,axes:l.axes,precision_bits:l.precision*8,sampled_curves:sampled,gamma_curves:gamma,parametric_curves:parametric,matrix:l.matrix.is_some(),m_curves:l.m.is_some(),structural_reorder:true,strict_sampled_bounds})
}
pub fn inspect(p:&Profile)->Vec<(String,R<Structure>)>{
 [b"A2B0",b"A2B1",b"A2B2",b"B2A0",b"B2A1",b"B2A2"].into_iter().filter_map(|k|p.tags.get(k).map(|t|{
  let key=String::from_utf8_lossy(k).into_owned();
  let r=if t.starts_with(b"mAB ")||t.starts_with(b"mBA "){describe(k,t)}
  else if t.starts_with(b"mft1")||t.starts_with(b"mft2"){
   crate::Lut::parse(t).map(|l|Structure{tag:key.clone(),kind:String::from_utf8_lossy(&t[..4]).into_owned(),inputs:l.inputs,outputs:l.outputs,axes:vec![l.grid;l.inputs],precision_bits:if t.starts_with(b"mft1"){8}else{16},sampled_curves:l.inputs+l.outputs,gamma_curves:0,parametric_curves:0,matrix:false,m_curves:false,structural_reorder:true,strict_sampled_bounds:k[0]==b'B'})
  }else{Err("Unsupported transform encoding".into())};
  (key,r)
 })).collect()
}
pub(crate) fn reorder(t:&[u8],forward:bool,order:&[usize])->R<Vec<u8>>{
 let l=Layout::parse(t)?;
 let n=if forward{l.inputs}else{l.outputs};
 if l.forward!=forward||order.len()!=n{return Err("Unexpected device LUT dimensions".into())}
 let mut seen=[false;15];for &j in order{if j>=n||seen[j]{return Err("Invalid channel permutation".into())}seen[j]=true;}
 let mut out=t.to_vec();
 if let Some(c)=l.device_curves(){
  let mut offset=c.ranges[0].start;
  for &j in order{let r=&c.ranges[j];out[offset..offset+r.len()].copy_from_slice(&t[r.clone()]);offset+=r.len();}
 }
 if forward{
  let new_axes=order.iter().map(|&j|l.axes[j]).collect::<Vec<_>>();
  let cl=l.clut.start-20;for (j,&g) in new_axes.iter().enumerate(){out[cl+j]=g as u8;}
  let width=l.outputs*l.precision;
  let mut old=vec![0;n];
  for index in 0..l.clut.len()/width{
   let mut q=index;
   for j in (0..n).rev(){old[order[j]]=q%new_axes[j];q/=new_axes[j];}
   let from=old.iter().zip(&l.axes).fold(0usize,|v,(&x,&g)|v*g+x);
   let a=l.clut.start+index*width;let b=l.clut.start+from*width;
   out[a..a+width].copy_from_slice(&t[b..b+width]);
  }
 }else{
  let width=n*l.precision;
  for (dst,src) in out[l.clut.clone()].chunks_exact_mut(width).zip(t[l.clut.clone()].chunks_exact(width)){
   for (j,&old) in order.iter().enumerate(){dst[j*l.precision..(j+1)*l.precision].copy_from_slice(&src[old*l.precision..(old+1)*l.precision]);}
  }
 }
 Ok(out)
}
/// A reverse CLUT and its sampled device-side post curves. PCS-side curves,
/// matrix, and M stages precede the CLUT in mBA; they cannot expand the
/// envelope of its output nodes. Parametric/gamma output bounds are deferred.
pub(crate) fn reverse_envelope(t:&[u8],n:usize)->R<(Vec<f64>,Vec<Vec<f64>>)>{
 let l=Layout::parse(t)?;
 if l.forward||l.inputs!=3||l.outputs!=n{return Err("Unexpected reverse LUT dimensions".into())}
 let post=if let Some(c)=l.device_curves(){
  if c.gamma!=0||c.parametric!=0{return Err("Analytic post-curve bounds not yet supported".into())}
  let mut post=Vec::with_capacity(n);
  for r in &c.ranges{
   let count=u32at(t,r.start+8)? as usize;
   if count==0{post.push(vec![0.,1.]);}
   else{post.push(t[r.start+12..r.start+12+2*count].chunks_exact(2).map(|x|u16::from_be_bytes([x[0],x[1]]) as f64/65535.).collect());}
  }
  post
 }else{vec![vec![0.,1.];n]};
 let values=if l.precision==1{t[l.clut].iter().map(|&x|x as f64/255.).collect()}else{t[l.clut].chunks_exact(2).map(|x|u16::from_be_bytes([x[0],x[1]]) as f64/65535.).collect()};
 Ok((values,post))
}
#[cfg(test)]
mod tests{
 use super::*;
 use crate::{put16,put32,identity_curves};
 fn unequal()->Vec<u8>{
  let mut t=b"mAB\0\0\0\0".to_vec();t.extend([4,3,0,0]);
  let cl=80usize;let end=cl+20+2*3*2*2*3*2;
  for v in [32,0,0,cl as u32,end as u32]{put32(&mut t,v);}
  identity_curves(&mut t,3);t.extend([2,3,2,2]);t.extend([0;12]);t.extend([2,0,0,0]);
  for i in 0..72{put16(&mut t,(i*773) as u16);}
  for j in 0..4{t.extend(b"para\0\0\0\0");put16(&mut t,0);put16(&mut t,0);put32(&mut t,(j+1)*65536);}
  t
 }
 #[test]fn unequal_grid_parametric_reorder_roundtrip(){
  let t=unequal();let order=[1,0,3,2];
  let v=reorder(&t,true,&order).unwrap();
  let a=Layout::parse(&t).unwrap();let b=Layout::parse(&v).unwrap();
  assert_eq!(b.axes,vec![3,2,2,2]);assert_eq!(b.a.as_ref().unwrap().parametric,4);
  assert_eq!(&t[a.b.as_ref().unwrap().ranges[0].clone()],&v[b.b.as_ref().unwrap().ranges[0].clone()]);
  assert_eq!(reorder(&v,true,&order).unwrap(),t);
 }
 #[test]fn reverse_matrix_and_m_curve_stages_are_untouched(){
  let p=crate::print_space::fixture(6);let mut t=p.tags[b"B2A1"].clone();
  let mo=t.len();for i in 0..12{put32(&mut t,if [0,4,8].contains(&i){65536}else{0});}
  let co=t.len();identity_curves(&mut t,3);t[16..20].copy_from_slice(&(mo as u32).to_be_bytes());t[20..24].copy_from_slice(&(co as u32).to_be_bytes());
  let v=reorder(&t,false,&[5,4,3,2,1,0]).unwrap();assert_eq!(&t[mo..],&v[mo..]);
  let (clut,_)=reverse_envelope(&v,6).unwrap();assert!(clut.chunks_exact(6).all(|x|x[0]==0.));
  assert_eq!(reorder(&v,false,&[5,4,3,2,1,0]).unwrap(),t);
 }
 #[test]fn reject_overlapping_or_truncated_stages(){
  let t=unequal();let mut v=t.clone();v[28..32].copy_from_slice(&32u32.to_be_bytes());assert!(Layout::parse(&v).is_err());
  assert!(Layout::parse(&t[..t.len()-1]).is_err());
 }
}
