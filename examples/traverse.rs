use clap::{App, Arg};
use wsdl::Wsdl;

fn main() -> anyhow::Result<()> {
    let matches = App::new("WSDL tree traversal example")
        .args(&[Arg::with_name("input")
            .takes_value(true)
            .required(true)
            .help("Input wsdl file")])
        .get_matches();

    let input = matches.value_of("input").unwrap();

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

        for operation in binding.operations() {
            println!("  {}: {}", operation.name()?, match operation.transport_operation()? {
                wsdl::WsBindingTransportOperation::Soap(action) => {
                    action
                }
                wsdl::WsBindingTransportOperation::Soap12(action) => {
                    action
                }
                wsdl::WsBindingTransportOperation::Http(location) => {
                    location
                }
                // _ => ""
            });
        }
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
                        s.push_str(&format!("{}: {}", part.name()?, part.typename()?));
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

                    part.typename()?.to_string()
                } else {
                    "()".to_string()
                }
            );
        }
    }

    Ok(())
}
