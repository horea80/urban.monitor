//! Nota de confidențialitate (FR-9): ce stocăm pentru conturi și de ce. Textul e un minim onest,
//! de completat de proprietar.

use dioxus::prelude::*;

use super::Route;
use crate::query::AccountParams;

#[component]
pub fn Privacy() -> Element {
    rsx! {
        h1 { "Confidențialitate" }
        p {
            "Monitor Urban publică informații deja publice: ședințele Comisiei Tehnice de Amenajare a Teritoriului și "
            "Urbanism (CTATU) a Primăriei Cluj-Napoca și proiectele de pe ordinea lor de zi, preluate de pe "
            "primariaclujnapoca.ro. Pentru căutare nu ai nevoie de cont și nu stocăm nimic despre tine."
        }
        h2 { "Dacă îți faci cont" }
        ul {
            li { "Stocăm adresa de email, data la care ți-ai dat acordul, cuvintele-cheie alese și un istoric al alertelor trimise (câte proiecte, când)." }
            li { "Le folosim doar pentru autentificare (linkul trimis pe email) și pentru alertele pe care le-ai cerut. Nu trimitem altceva și nu dăm adresa mai departe." }
            li { "Emailurile pleacă prin Resend (furnizor de trimitere email); serverul e administrat de noi, pe un VPS închiriat." }
            li { "Sesiunea de autentificare e un cookie tehnic, necesar, valabil 30 de zile; nu folosim cookie-uri de urmărire." }
            li {
                "Poți șterge contul oricând din "
                Link { to: Route::Account { params: AccountParams::default() }, "pagina contului" }
                "; ștergerea elimină imediat adresa, cuvintele-cheie și istoricul."
            }
        }
        p { class: "muted", "Întrebări despre datele tale: scrie la adresa de contact a site-ului." }
    }
}
