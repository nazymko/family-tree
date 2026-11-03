//! Event tag tables shared by import and export.

use gt_core::model::EventKind;

/// Event kind for a GEDCOM tag (`EVEN`/`FACT` are handled by the caller).
pub fn event_kind(tag: &str) -> Option<EventKind> {
    use EventKind::*;
    Some(match tag {
        "BIRT" => Birth,
        "CHR" => Christening,
        "BAPM" => Baptism,
        "DEAT" => Death,
        "BURI" => Burial,
        "CREM" => Cremation,
        "ADOP" => Adoption,
        "MARR" => Marriage,
        "MARB" => MarriageBanns,
        "MARC" => MarriageContract,
        "MARL" => MarriageLicense,
        "MARS" => MarriageSettlement,
        "ENGA" => Engagement,
        "DIV" => Divorce,
        "DIVF" => DivorceFiled,
        "ANUL" => Annulment,
        "_SEPR" => Separation,
        "CENS" => Census,
        "RESI" => Residence,
        "OCCU" => Occupation,
        "EDUC" => Education,
        "RELI" => Religion,
        "NATI" => Nationality,
        "TITL" => Title,
        "EMIG" => Emigration,
        "IMMI" => Immigration,
        "NATU" => Naturalization,
        "PROB" => Probate,
        "WILL" => Will,
        "GRAD" => Graduation,
        "RETI" => Retirement,
        "_MILT" | "_MILI" => Military,
        "DSCR" => PhysicalDescription,
        _ => return None,
    })
}

/// GEDCOM tag for an event kind. `Custom` yields `EVEN` (5.5.1) or `FACT` (7.0).
pub fn event_tag(kind: &EventKind, v70: bool) -> &'static str {
    use EventKind::*;
    match kind {
        Birth => "BIRT",
        Christening => "CHR",
        Baptism => "BAPM",
        Death => "DEAT",
        Burial => "BURI",
        Cremation => "CREM",
        Adoption => "ADOP",
        Marriage => "MARR",
        MarriageBanns => "MARB",
        MarriageContract => "MARC",
        MarriageLicense => "MARL",
        MarriageSettlement => "MARS",
        Engagement => "ENGA",
        Divorce => "DIV",
        DivorceFiled => "DIVF",
        Annulment => "ANUL",
        Separation => "_SEPR",
        Census => "CENS",
        Residence => "RESI",
        Occupation => "OCCU",
        Education => "EDUC",
        Religion => "RELI",
        Nationality => "NATI",
        Title => "TITL",
        Emigration => "EMIG",
        Immigration => "IMMI",
        Naturalization => "NATU",
        Probate => "PROB",
        Will => "WILL",
        Graduation => "GRAD",
        Retirement => "RETI",
        Military => "_MILT",
        PhysicalDescription => "DSCR",
        Custom(_) => {
            if v70 {
                "FACT"
            } else {
                "EVEN"
            }
        }
    }
}
