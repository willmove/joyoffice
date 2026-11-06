fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut total = 0;
    let mut changed = 0;
    for (index, arg) in std::env::args().skip(1).enumerate() {
        let doc = joyword_docx::read(&std::fs::read(&arg)?)?;
        let saved = joyword_docx::write(&doc)?;
        if let Ok(dir) = std::env::var("JOYWORD_STYLE_AUDIT_OUT") {
            std::fs::write(
                std::path::Path::new(&dir).join(format!("{index:02}.docx")),
                &saved,
            )?;
        }
        let out = joyword_docx::read(&saved)?;
        for st in &doc.styles.styles {
            let mut c = joyword_model::props::CharProps::default();
            let para_style = match st.kind {
                joyword_model::styles::StyleKind::Paragraph => Some(st.id.as_str()),
                joyword_model::styles::StyleKind::Character => {
                    c.style = Some(st.id.clone());
                    None
                }
                _ => continue,
            };
            if std::env::var_os("JOYWORD_STYLE_AUDIT_VALUES").is_some() {
                let resolved = doc.styles.resolve_char(para_style, &c);
                println!(
                    "VALUE {} font={:?} color={:?}",
                    st.id, resolved.font, resolved.color
                );
            }
            let a = doc.styles.resolve_char(para_style, &c);
            let b = out.styles.resolve_char(para_style, &c);
            let p = joyword_model::props::ParaProps {
                style: para_style.map(str::to_string),
                ..Default::default()
            };
            let pa = doc.styles.resolve_para(&p);
            let pb = out.styles.resolve_para(&p);
            total += 1;
            if a != b || pa != pb {
                changed += 1;
                println!(
                    "DIFFERENCE {arg} {}\nCHAR_BEFORE {a:?}\nCHAR_AFTER {b:?}\nPARA_BEFORE {pa:?}\nPARA_AFTER {pb:?}",
                    st.id
                );
            }
        }
        println!("FILE {arg} styles={}", doc.styles.styles.len());
    }
    println!("TOTAL styles={total} changed={changed}");
    Ok(())
}
