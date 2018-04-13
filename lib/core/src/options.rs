//! # Helper data structure do handle option lookups

use {Diagnostics, Loc, OptionEntry, RpNumber, WithSpan};

/// Helper for looking up and dealing with options.
pub trait Options {
    type Item: OptionEntry;

    fn items(&self) -> &Vec<Loc<Self::Item>>;

    fn lookup(&self, name: &str) -> Vec<Loc<&Self::Item>> {
        self.items()
            .iter()
            .filter(move |o| o.name() == name)
            .map(|option| Loc::as_ref(&option))
            .collect()
    }

    /// Find all strings matching the given name.
    ///
    /// This enforces that all found values are strings, otherwise the lookup will cause an error.
    fn find_all_strings(&self, diag: &mut Diagnostics, name: &str) -> Result<Vec<Loc<String>>, ()> {
        let mut out = Vec::new();

        for s in self.lookup(name) {
            let (value, span) = Loc::take_pair(s);
            let string = value.as_string().with_span(diag, &span)?;
            out.push(Loc::new(string, span));
        }

        Ok(out)
    }

    fn find_all_u32(&self, diag: &mut Diagnostics, name: &str) -> Result<Vec<Loc<RpNumber>>, ()> {
        let mut out = Vec::new();

        for s in self.lookup(name) {
            let (value, span) = Loc::take_pair(s);
            let number = value.as_number().with_span(diag, &span)?;
            out.push(Loc::new(number, span));
        }

        Ok(out)
    }

    /// Find all identifiers matching the given name.
    ///
    /// This enforces that all found values are identifiers, otherwise the lookup will cause an
    /// error.
    fn find_all_identifiers(
        &self,
        diag: &mut Diagnostics,
        name: &str,
    ) -> Result<Vec<Loc<String>>, ()> {
        let mut out = Vec::new();

        for s in self.lookup(name) {
            let (value, span) = Loc::take_pair(s);
            let identifier = value.as_identifier().with_span(diag, &span)?;
            out.push(Loc::new(identifier, span));
        }

        Ok(out)
    }

    /// Optionally find exactly one identifier matching the given name.
    ///
    /// This enforces that all found values are identifiers, otherwise the lookup will cause an
    /// error.
    fn find_one_identifier(
        &self,
        diag: &mut Diagnostics,
        name: &str,
    ) -> Result<Option<Loc<String>>, ()> {
        Ok(self.find_all_identifiers(diag, name)?.into_iter().next())
    }

    fn find_one_string(
        &self,
        diag: &mut Diagnostics,
        name: &str,
    ) -> Result<Option<Loc<String>>, ()> {
        Ok(self.find_all_strings(diag, name)?.into_iter().next())
    }

    fn find_one_u32(
        &self,
        diag: &mut Diagnostics,
        name: &str,
    ) -> Result<Option<Loc<RpNumber>>, ()> {
        Ok(self.find_all_u32(diag, name)?.into_iter().next())
    }
}

impl<T> Options for Vec<Loc<T>>
where
    T: OptionEntry,
{
    type Item = T;

    fn items(&self) -> &Vec<Loc<Self::Item>> {
        self
    }
}
