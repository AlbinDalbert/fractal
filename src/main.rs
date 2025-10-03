mod frac;
mod fractal;

fn main() {
    println!("Fractal CLI running!");

    let doc = frac::IntermediateRep {
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