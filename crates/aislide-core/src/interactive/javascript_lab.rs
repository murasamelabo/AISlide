use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::{Error, Result};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Kind { Simulator, Network, Priority, Playback, Comparison, Quiz, Explorer, Canvas }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub slide_id:String,
    pub element_id:String,
    pub kind:Kind,
}

pub(super) fn html(kind:Kind,title:&str,source:&str,width:f64,height:f64)->Result<String> {
    if !width.is_finite() || !height.is_finite() || !(960.0..=4096.0).contains(&width) || !(480.0..=4096.0).contains(&height) {
        return Err(Error::Invalid("JavaScript lab canvas requires finite native dimensions of at least 960x480 and at most 4096x4096".into()));
    }
    let runtime=include_str!("../../../../apps/office-content/lab-runtime.mjs").trim_start_matches('\u{feff}');
    let css=include_str!("../../../../apps/office-content/lab.css").trim_start_matches('\u{feff}');
    let declaration="export function mountJavascriptLab";
    if runtime.matches(declaration).count()!=1 || runtime.to_ascii_lowercase().contains("</script") || css.to_ascii_lowercase().contains("</style") {
        return Err(Error::Invalid("trusted JavaScript lab runtime cannot be embedded safely".into()));
    }
    let runtime=runtime.replacen(declaration,"function mountJavascriptLab",1);
    let payload=serde_json::to_string(&json!({"version":1,"kind":kind,"title":title,"source":source,"canvas":{"width":width,"height":height}}))?
        .replace('&',"\\u0026").replace('<',"\\u003c").replace('>',"\\u003e").replace('\u{2028}',"\\u2028").replace('\u{2029}',"\\u2029");
    let html=format!(r#"<!doctype html><html lang="ja"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'"><title>AISlide JavaScript表現実験</title><style>{css}</style></head><body><main id="interactive-root"></main><script>(()=>{{"use strict";{runtime}
try {{ mountJavascriptLab(document.getElementById("interactive-root"),{payload}); }}
catch(reason) {{const message=document.createElement("p");message.setAttribute("role","alert");message.textContent="JavaScript実験を開始できません: "+(reason instanceof Error?reason.message:String(reason));document.getElementById("interactive-root").replaceChildren(message);}}
}})();</script></body></html>"#);
    if html.len()>2*1024*1024 {return Err(Error::Limit("JavaScript lab HTML exceeds 2 MiB".into()));}
    Ok(html)
}
