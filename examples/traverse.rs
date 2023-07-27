use clap::{Arg, Command};
use wsdl::Wsdl;

fn main() -> anyhow::Result<()> {
    let matches = Command::new("WSDL tree traversal example")
        .args(&[Arg::new("input")
            .num_args(1)
            .required(true)
            .help("Input wsdl file")])
        .get_matches();

    let input = matches.get_one::<String>("input").unwrap();

    let input = std::fs::read_to_string(&input)?;
    let document = roxmltree::Document::parse(&input)?;

    let wsdl = Wsdl::new(&document);
    for service in wsdl.services()? {
        println!("Service: {}", service.name()?);
    }

    for binding in wsdl.bindings()? {
        println!(
            "Binding: {} -> {}",
            binding.name()?,
            binding.port_type()?.name()?
        );
    }

    for port_type in wsdl.port_types()? {
        println!("Port: {}", port_type.name()?);

        for operation in port_type.operations()? {
            println!(
                "  {}({}) -> {}",
                operation.name()?,
                if let Some(input) = operation.input()? {
                    let mut s = String::new();

                    for part in input.parts() {
                        let tn = part.typename()?;
                        let tn = if let Some(ns) = tn.namespace() {
                            format!("`{}:{}`", ns, tn.name())
                        } else {
                            format!("{}", tn.name())
                        };

                        s.push_str(&format!("{}: {}", part.name()?, tn));
                    }

                    s
                } else {
                    "".to_string()
                },
                if let Some(output) = operation.output()? {
                    let part = output.parts().next().ok_or(anyhow::anyhow!(
                        "message {} output had no parameters!",
                        operation.name().unwrap()
                    ))?;

                    let tn = part.typename()?;
                    if let Some(ns) = tn.namespace() {
                        format!("`{}:{}`", ns, tn.name())
                    } else {
                        format!("{}", tn.name())
                    }
                } else {
                    "()".to_string()
                }
            );
        }
    }

    Ok(())
}
