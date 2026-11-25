//! Studio releases use a separate namespace; legacy converter releases are never candidates.
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub version: String,
    pub release_url: String,
    pub download_url: String,
    pub checksum_url: Option<String>,
    pub asset_digest: Option<String>,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    html_url: String,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}
fn version(s: &str) -> Option<[u32; 5]> {
    let (core, beta) = s.split_once("-beta.")?;
    let mut a = core.split('.');
    let mut b = beta.split('.');
    let n = [a.next()?.parse().ok()?, a.next()?.parse().ok()?, a.next()?.parse().ok()?, b.next()?.parse().ok()?, match b.next() { Some(v) => v.parse().ok()?, None => 0 }];
    if a.next().is_some() || b.next().is_some() { return None; }
    Some(n)
}
fn digest_ok(s: &str) -> bool {
    s.strip_prefix("sha256:").is_some_and(|v| v.len()==64 && v.bytes().all(|c|c.is_ascii_hexdigit()))
}
pub fn select(body: &[u8], current: &str) -> Result<Option<Candidate>, String> {
    let current = version(current).ok_or("Invalid current Studio beta version")?;
    let releases: Vec<Release> = serde_json::from_slice(body).map_err(|e|format!("Invalid release metadata: {e}"))?;
    let mut best: Option<([u32;5],Candidate)> = None;
    for r in releases {
        if r.draft || !r.prerelease { continue; }
        let Some(v) = r.tag_name.strip_prefix("studio-v") else {continue};
        let Some(n) = version(v) else {continue};
        if n <= current || best.as_ref().is_some_and(|(b,_)| n<=*b) {continue}
        let prefix = format!("https://github.com/emadgh/color-convert-rust/releases/download/{}/",r.tag_name);
        let release_url = format!("https://github.com/emadgh/color-convert-rust/releases/tag/{}",r.tag_name);
        if r.html_url != release_url {continue}
        let Some(exe) = r.assets.iter().find(|a|a.name=="ICC-Profile-Studio.exe" && a.browser_download_url==format!("{prefix}ICC-Profile-Studio.exe")) else {continue};
        let checksum_url = r.assets.iter().find(|a|a.name=="ICC-Profile-Studio.exe.sha256" && a.browser_download_url==format!("{prefix}ICC-Profile-Studio.exe.sha256")).map(|a|a.browser_download_url.clone());
        let asset_digest = exe.digest.clone().filter(|d|digest_ok(d));
        if asset_digest.is_none() && checksum_url.is_none() {continue}
        best=Some((n,Candidate{version:v.to_owned(),release_url,download_url:exe.browser_download_url.clone(),checksum_url,asset_digest}));
    }
    Ok(best.map(|(_,c)|c))
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn release(tag:&str,checksum:bool,draft:bool)->serde_json::Value {
        let base=format!("https://github.com/emadgh/color-convert-rust/releases");
        let mut assets=vec![json!({"name":"ICC-Profile-Studio.exe","browser_download_url":format!("{base}/download/{tag}/ICC-Profile-Studio.exe")})];
        if checksum {assets.push(json!({"name":"ICC-Profile-Studio.exe.sha256","browser_download_url":format!("{base}/download/{tag}/ICC-Profile-Studio.exe.sha256")}));}
        json!({"tag_name":tag,"draft":draft,"prerelease":true,"html_url":format!("{base}/tag/{tag}"),"assets":assets})
    }
    #[test] fn select_only_newer_complete_studio_beta(){
        let rows=vec![release("v99.0.0-beta.1",true,false),release("studio-v0.9.0-beta.29",true,false),release("studio-v0.9.0-beta.30",false,false),release("studio-v0.9.0-beta.29.2",true,false),release("studio-v0.9.0-beta.99",true,true)];
        let result=select(&serde_json::to_vec(&rows).unwrap(),"0.9.0-beta.28.1").unwrap().unwrap();
        assert_eq!(result.version,"0.9.0-beta.29.2");
        assert!(select(&serde_json::to_vec(&rows).unwrap(),"0.9.0-beta.29.2").unwrap().is_none());
    }
    #[test] fn numeric_order_and_namespace(){assert!(version("0.9.0-beta.10")>version("0.9.0-beta.9"));assert!(version("0.9.0-beta.1.extra").is_none());assert!(version("0.9.0").is_none());}
    #[test] fn rejects_malformed_feed_and_unverified_assets(){
        assert!(select(b"not json","0.9.0-beta.40").is_err());
        let mut r=release("studio-v0.9.0-beta.41",false,false);
        r["assets"][0]["digest"]=json!("sha256:bad");
        assert!(select(&serde_json::to_vec(&vec![r.clone()]).unwrap(),"0.9.0-beta.40").unwrap().is_none());
        r["assets"][0]["digest"]=json!(format!("sha256:{}","a".repeat(64)));
        let c=select(&serde_json::to_vec(&vec![r]).unwrap(),"0.9.0-beta.40").unwrap().unwrap();
        assert!(c.asset_digest.is_some());assert!(c.checksum_url.is_none());
    }
    #[test] fn rejects_bad_asset_url(){
        let mut r=release("studio-v0.9.0-beta.29",true,false);
        r["assets"][0]["browser_download_url"]=json!("https://example.com/ICC-Profile-Studio.exe");
        assert!(select(&serde_json::to_vec(&vec![r]).unwrap(),"0.9.0-beta.28").unwrap().is_none());
    }
}
