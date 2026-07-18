//! Self-contained Markdown export of a session (PLAN §11). Interpolated fields are
//! HTML-escaped by default so hostile provider/user content cannot become
//! executable HTML if the exported file is later rendered.

use krunch_core::ids::{SeatId, SessionId};
use krunch_store::Store;

/// Escape the characters that could turn interpolated text into active HTML.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn short(seat: SeatId) -> String {
    seat.to_string()[..8].to_string()
}

/// Assemble the full Markdown transcript + verdict for a session.
pub async fn export_markdown(store: &Store, session: SessionId) -> Result<String, String> {
    let e = |r: krunch_store::StoreError| r.to_string();

    let summary = store.get_session(session).await.map_err(e)?;
    let seats = store.seats(session).await.map_err(e)?;
    let rounds = store.rounds(session).await.map_err(e)?;
    let qa = store.user_qa(session).await.map_err(e)?;

    let mediator = seats.iter().find(|s| s.role == "mediator");
    let panelists: Vec<_> = seats.iter().filter(|s| s.role == "panelist").collect();

    // Display-name lookup.
    let name_of = |id: SeatId| -> String {
        seats
            .iter()
            .find(|s| s.seat == id)
            .map(|s| s.display_name.clone())
            .unwrap_or_else(|| short(id))
    };

    let outcome = format!("{:?}", summary.state);
    let deliberation_rounds: Vec<_> = rounds.iter().filter(|r| r.kind == "deliberation").collect();
    let finalization = rounds.iter().find(|r| r.kind == "finalization");

    let mut out = String::new();

    // --- header ---
    out.push_str(&format!("# krunch deliberation — {outcome}\n\n"));
    out.push_str("## Problem\n\n");
    out.push_str(&esc(&summary.problem));
    out.push_str("\n\n");
    out.push_str(&format!(
        "- **Outcome:** {outcome}\n- **Deliberation rounds:** {}\n- **Round cap:** {}\n\n",
        deliberation_rounds.len(),
        summary.max_rounds
    ));

    // --- roster (audit snapshot) ---
    out.push_str("## Panel (audit snapshot)\n\n");
    if let Some(m) = mediator {
        out.push_str(&format!(
            "- **Mediator** — {} · `{}` · {}\n",
            esc(&m.display_name),
            esc(&m.model),
            esc(&m.provider)
        ));
    }
    for p in &panelists {
        out.push_str(&format!(
            "- Panelist — {} · `{}` · {}\n",
            esc(&p.display_name),
            esc(&p.model),
            esc(&p.provider)
        ));
    }
    out.push('\n');

    // --- verdict ---
    out.push_str("## Verdict\n\n");
    if let (Some(fin), Some(m)) = (finalization, mediator) {
        if let Some(text) = store.accepted_text(fin.id, m.seat).await.map_err(e)? {
            out.push_str(&esc(&text));
            out.push_str("\n\n");
        } else {
            out.push_str("_No verdict was synthesized._\n\n");
        }
    } else {
        out.push_str(&format!("_Deliberation ended in `{outcome}` without a synthesized verdict._\n\n"));
    }

    // --- aggregated assumptions ---
    let mut assumptions: Vec<String> = Vec::new();
    for r in &deliberation_rounds {
        if let Some(ruling) = store.ruling(r.id).await.map_err(e)? {
            assumptions.extend(ruling.assumptions);
        }
    }
    if !assumptions.is_empty() {
        out.push_str("## Assumptions made\n\n");
        for a in &assumptions {
            out.push_str(&format!("- {}\n", esc(a)));
        }
        out.push('\n');
    }

    // --- transcript ---
    out.push_str("## Transcript\n\n");
    for r in &deliberation_rounds {
        out.push_str(&format!("### Round {}\n\n", r.index_no + 1));
        if let Some(focus) = &r.focus {
            if !focus.trim().is_empty() {
                out.push_str(&format!("_Focus: {}_\n\n", esc(focus)));
            }
        }

        let stances = store.stances(r.id).await.map_err(e)?;
        for p in &panelists {
            out.push_str(&format!("#### {}\n\n", esc(&p.display_name)));
            if let Some(text) = store.accepted_text(r.id, p.seat).await.map_err(e)? {
                out.push_str(&esc(&text));
                out.push_str("\n\n");
            } else {
                out.push_str("_(abstained)_\n\n");
            }
            if let Some(st) = stances.iter().find(|s| s.seat == p.seat) {
                let agree: Vec<String> = st.agree_with.iter().map(|a| name_of(*a)).collect();
                out.push_str(&format!(
                    "> **Stance:** {} · confidence {:.2}{}\n\n",
                    esc(&st.stance),
                    st.confidence,
                    if agree.is_empty() {
                        String::new()
                    } else {
                        format!(" · agrees with {}", esc(&agree.join(", ")))
                    }
                ));
            }
        }

        if let Some(ruling) = store.ruling(r.id).await.map_err(e)? {
            out.push_str(&format!("**Mediator — ruling: {}**\n\n", esc(&ruling.ruling)));
            if !ruling.summary.trim().is_empty() {
                out.push_str(&esc(&ruling.summary));
                out.push_str("\n\n");
            }
        }

        // User Q&A that resolved at this round boundary.
        for exchange in qa.iter().filter(|q| q.round_index == r.index_no) {
            out.push_str(&format!(
                "**User Q&A:**\n\n> Q: {}\n>\n> A: {}\n\n",
                esc(&exchange.question),
                esc(&exchange.answer)
            ));
        }
    }

    Ok(out)
}
