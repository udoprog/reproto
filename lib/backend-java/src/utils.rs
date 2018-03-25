use genco::java::{imported, local, Argument, Interface, Method};
use genco::{IntoTokens, Java, Tokens};

/// @Override annotation
pub struct Override;

impl<'el> IntoTokens<'el, Java<'el>> for Override {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        toks!["@Override"]
    }
}

/// Observer interface used for bidirectional streaming communication.
pub struct Observer;

impl<'el> IntoTokens<'el, Java<'el>> for Observer {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        let mut c = Interface::new("Observer");
        let v = local("V");

        c.parameters.append(v.clone());

        let throwable = imported("java.lang", "Throwable");

        c.methods.push({ Method::new("onCompleted") });
        c.methods.push({
            let mut m = Method::new("onError");
            m.arguments.push(Argument::new(throwable.clone(), "error"));
            m
        });
        c.methods.push({
            let mut m = Method::new("onNext");
            m.arguments.push(Argument::new(v.clone(), "value"));
            m
        });

        c.into_tokens()
    }
}
