use fractal::frac::IntermediateRep;

fn main() {
    println!("Fractal CLI running!");

    let doc = IntermediateRep {
        size: 0,
        body: vec![],
        count: 0,
        last_modified: None,
        author: None,
        title: "MyProject".to_string(),
        checksum: None,
        tags: None,
        footnotes: None,
    };

    println!("Loaded empty document: {:?}", doc.title);
}

fn frac_to_md(frac: IntermediateRep) {
    todo!()
}

fn md_to_frac(md_path: String) {
    todo!()
}

fn generate_new_fractal(path: String) {
    todo!()
}