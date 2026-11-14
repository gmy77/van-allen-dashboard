//! Testi di aiuto dei riquadri, mostrati dal pulsante "?".
//!
//! Ogni voce dice tre cose: cosa mostra il riquadro, come si legge il numero,
//! e se quel numero è **misura** o **modello**. La distinzione è la stessa del
//! resto del progetto: nessun modello viene presentato come osservazione.
//!
//! Nei testi `**così**` diventa grassetto e una riga vuota separa i paragrafi.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Topic {
    // generale
    Guide,
    // scheda Fasce
    Kp,
    Dst,
    Wind,
    Bz,
    Magnetopause,
    Electrons,
    Strip,
    Profile,
    Spectrum,
    Series,
    Alerts,
    // scheda Vento solare
    Speed,
    Density,
    Temperature,
    Bt,
    Pressure,
    ImfSeries,
    // scheda Magnetosfera
    ScaleR,
    ScaleS,
    ScaleG,
    Hp,
    Protons,
    Plasmapause,
    KpSeries,
    DstSeries,
    // scheda Storico
    HistFlux,
    HistKp,
    HistDst,
    HistMagnetopause,
    HistBelt,
    HistContent,
    // scheda Sorgenti
    Sources,
    Server,
}

impl Topic {
    /// Tutte le voci, nell'ordine in cui compaiono nell'interfaccia.
    pub const ALL: [Topic; 34] = [
        Self::Guide,
        Self::Kp,
        Self::Dst,
        Self::Wind,
        Self::Bz,
        Self::Magnetopause,
        Self::Electrons,
        Self::Strip,
        Self::Profile,
        Self::Spectrum,
        Self::Series,
        Self::Alerts,
        Self::Speed,
        Self::Density,
        Self::Temperature,
        Self::Bt,
        Self::Pressure,
        Self::ImfSeries,
        Self::ScaleR,
        Self::ScaleS,
        Self::ScaleG,
        Self::Hp,
        Self::Protons,
        Self::Plasmapause,
        Self::KpSeries,
        Self::DstSeries,
        Self::HistFlux,
        Self::HistKp,
        Self::HistDst,
        Self::HistMagnetopause,
        Self::HistBelt,
        Self::HistContent,
        Self::Sources,
        Self::Server,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Guide => "Come si usa la dashboard",
            Self::Kp => "Kp stimato",
            Self::Dst => "Dst",
            Self::Wind => "Vento solare",
            Self::Bz => "Bz e pressione dinamica",
            Self::Magnetopause => "Magnetopausa",
            Self::Electrons => "Elettroni > 2 MeV a L = 6.6",
            Self::Strip => "Sezione radiale delle fasce",
            Self::Profile => "Profilo radiale del flusso",
            Self::Spectrum => "Spettro differenziale",
            Self::Series => "Serie delle ultime 24 ore",
            Self::Alerts => "Allerte e stato",
            Self::Speed => "Velocità del vento solare",
            Self::Density => "Densità del vento solare",
            Self::Temperature => "Temperatura dei protoni",
            Self::Bt => "Intensità del campo interplanetario (Bt)",
            Self::Pressure => "Pressione dinamica",
            Self::ImfSeries => "Campo interplanetario: Bz e Bt",
            Self::ScaleR => "Scala R — blackout radio",
            Self::ScaleS => "Scala S — tempesta di radiazione",
            Self::ScaleG => "Scala G — tempesta geomagnetica",
            Self::Hp => "Campo magnetico a GEO (Hp)",
            Self::Protons => "Protoni solari",
            Self::Plasmapause => "Plasmapausa",
            Self::KpSeries => "Kp stimato nelle ultime ore",
            Self::DstSeries => "Dst degli ultimi 7 giorni",
            Self::HistFlux => "Storico: elettroni > 2 MeV",
            Self::HistKp => "Storico: Kp",
            Self::HistDst => "Storico: Dst",
            Self::HistMagnetopause => "Storico: magnetopausa",
            Self::HistBelt => "Storico: posizione della fascia",
            Self::HistContent => "Storico: contenuto della fascia",
            Self::Sources => "Stato delle sorgenti",
            Self::Server => "Server locale e file",
        }
    }

    pub fn body(self) -> &'static str {
        match self {
            Self::Guide => "\
La dashboard ha cinque schede. **Fasce** è il quadro d'insieme: dove sono le \
fasce di Van Allen, quanto sono intense e se c'è qualcosa fuori soglia. \
**Vento solare** mostra ciò che sta arrivando dal Sole, misurato al punto L1. \
**Magnetosfera** mostra come sta rispondendo la Terra: Kp, Dst, campo a GEO, \
protoni. **Storico** conserva l'andamento nel tempo, salvato sul PC. \
**Sorgenti** dice se ogni fonte di dati sta rispondendo.\n\n\
Ogni riquadro ha un **?** in alto a destra che spiega cosa mostra, come si \
legge e se il numero è una **misura** (registrata da un sensore) o un \
**modello** (calcolato qui, quindi con incertezza). Sui grafici delle schede \
Vento solare, Magnetosfera e Storico basta passare col puntatore per leggere \
il valore in quel momento.\n\n\
I dati si aggiornano da soli ogni 5 minuti; **Aggiorna ora** forza subito un \
nuovo scaricamento. Tutti gli orari sono UTC (in Italia: +1 ora d'inverno, \
+2 d'estate). R⊕ è il raggio terrestre, 6371 km.",

            Self::Kp => "\
Indice planetario dell'attività geomagnetica, da 0 a 9. 0–2 quieto, 3 \
perturbato, 4 attivo, da 5 in su tempesta (scala G1–G5).\n\n\
Il numero qui è la **stima NOAA al minuto**, ricavata in tempo quasi reale dai \
magnetometri a terra: è disponibile subito ma può differire dal Kp definitivo, \
che è calcolato ogni 3 ore su 13 osservatori ed esce più tardi.\n\n\
Sotto al numero c'è la scala G corrispondente.\n\n\
**Misura** (stima rapida).",

            Self::Dst => "\
Abbassamento del campo geomagnetico all'equatore, in nanotesla, calcolato dal \
centro dati di Kyoto su quattro osservatori a bassa latitudine. Valori negativi \
indicano una **corrente ad anello** intensa: particelle cariche intrappolate \
che girano attorno alla Terra durante le tempeste.\n\n\
Soglie usate qui: sopra −20 nT quieto, −20…−30 perturbato, −30…−50 tempesta \
debole, −50…−100 moderata, −100…−200 forte, sotto −200 severa. La scritta \
sotto al numero è la classe corrente.\n\n\
Il valore è l'ultimo dato orario di Kyoto (provvisorio, può essere ritoccato \
nei giorni successivi).\n\n\
**Misura.**",

            Self::Wind => "\
Velocità e densità del plasma solare misurate al punto L1, circa 1,5 milioni di \
km dalla Terra in direzione del Sole: è ciò che raggiungerà la magnetosfera \
entro 30–60 minuti.\n\n\
Tipico: 300–500 km/s e 3–10 protoni per cm³. Oltre ~600–700 km/s è vento \
veloce (buchi coronali o espulsioni di massa); una densità molto alta indica \
compressione, tipica del fronte di uno shock.\n\n\
Il dettaglio nel tempo è nella scheda **Vento solare**.\n\n\
**Misura.**",

            Self::Bz => "\
Bz è la componente nord-sud del campo magnetico interplanetario, in nanotesla \
(sistema GSM). **Bz negativo** (verso sud) è la condizione che accoppia il \
vento solare alla magnetosfera: lascia entrare energia. Il numero diventa \
arancione sotto −5 nT.\n\n\
P è la pressione dinamica del vento, P = 1,67·10⁻⁶ · n · V² in nPa. Valori \
tipici 1–3 nPa; sopra 5–10 nPa la magnetosfera si comprime in modo sensibile.\n\n\
Bz è **misura** (L1); la pressione è un calcolo diretto dalle misure.",

            Self::Magnetopause => "\
Distanza, in raggi terrestri e in chilometri, del confine sul lato del Sole \
dove la pressione del vento solare bilancia quella del campo geomagnetico. \
Oltre quel punto il campo terrestre non trattiene più le particelle.\n\n\
Calcolata con la formula di Shue (1998) da Bz e pressione dinamica. Tipico: \
10–11 R⊕. Errore dell'ordine di 1 R⊕. Se scende verso 6,6 R⊕ i satelliti \
geostazionari restano scoperti e il numero diventa rosso.\n\n\
**Modello**, non misura.",

            Self::Electrons => "\
Flusso integrale di elettroni con energia sopra 2 MeV misurato dal satellite \
GOES in orbita geostazionaria (L ≈ 6,6), in pfu = particelle per cm²·s·sr.\n\n\
Regola pratica: sotto 100 pfu fascia tranquilla, 100–1000 in crescita, oltre \
1000 pfu (numero giallo) soglia di allerta NOAA per la carica elettrostatica \
dei satelliti, oltre 10 000 (rosso) evento severo.\n\n\
È il dato a cui è ancorato tutto il modello delle fasce.\n\n\
**Misura.**",

            Self::Strip => "\
La fascia vista di taglio lungo l'equatore, da 1 a 8 raggi terrestri. Il colore \
è l'intensità del flusso di elettroni > 2 MeV: blu debole, verde medio, giallo \
forte, rosso molto forte.\n\n\
Le tacche verdi sono i bordi stimati della fascia esterna; quella azzurra è la \
plasmapausa (dentro di essa il plasma freddo svuota la fascia e crea lo \
«slot», il vuoto fra fascia interna ed esterna); quella gialla è la posizione \
di GOES, quella rossa la magnetopausa quando rientra nel grafico. Il \
rettangolo blu a sinistra è la Terra.\n\n\
Nel sottotitolo: bordi, ampiezza e contenuto (unità arbitrarie, da confrontare \
solo con sé stesso nel tempo).\n\n\
Forma e posizione: **modello** ancorato al flusso **misurato** a L = 6,6.",

            Self::Profile => "\
Lo stesso flusso della striscia, ma come curva in funzione di L (distanza \
equatoriale in raggi terrestri), su scala logaritmica.\n\n\
Si leggono tre cose: il picco (dove la fascia è più intensa), la larghezza e \
i bordi (dove il flusso scende sotto 100 pfu). Il regime riassume lo stato: \
stabili, perturbate, iniezione, slot eroso, dropout.\n\n\
La banda viola a sinistra è la fascia interna di protoni: ha forma \
climatologica, perché GOES non scende abbastanza per misurarla. La barra \
colorata a destra è la legenda dell'intensità.\n\n\
Il valore a L = 6,6 è **misurato**; il resto della forma è **modello**.",

            Self::Spectrum => "\
Flusso differenziale di elettroni nei canali di GOES, da 79 keV a 2,9 MeV, in \
pfu per MeV, su assi logaritmici: a sinistra gli elettroni poco energetici e \
numerosi, a destra quelli relativistici e rari.\n\n\
La curva arancione è adesso, quella grigia è l'inizio della finestra di 6 ore: \
se il presente sta sopra, il flusso è cresciuto. γ è la pendenza media: più è \
negativo, più lo spettro è ripido (pochi elettroni di alta energia); se si \
avvicina a zero lo spettro è duro, cioè ricco di elettroni relativistici.\n\n\
I punti sono **misura**; γ e flusso di energia sono calcolati dalla misura.",

            Self::Series => "\
Le ultime 24 ore di flusso > 2 MeV a L = 6,6, per vedere se la fascia sta \
salendo o scendendo. La linea tratteggiata è la soglia di allerta di 1000 pfu.\n\n\
Nel sottotitolo c'è la pendenza in decadi all'ora: sopra +0,15 sale, sotto \
−0,15 scende, in mezzo è stabile. Un'oscillazione lenta nell'arco della \
giornata è normale (il satellite passa dal lato giorno al lato notte). Un \
crollo improvviso è un «dropout», tipico delle tempeste: la fascia si svuota e \
poi si ricarica.\n\n\
**Misura.**",

            Self::Alerts => "\
Soglie operative, dalla più grave alla meno grave. Ogni riga dice cosa è \
successo e perché conta. Il colore della barra a sinistra indica il livello: \
azzurro informazione, giallo attenzione, arancione allerta, rosso grave.\n\n\
Sono regole ragionate su misure e modello: tempesta G1–G5, Dst, elettroni \
relativistici, magnetopausa dentro GEO, dropout, erosione dello slot, \
iniezioni o crolli rapidi, protoni solari.\n\n\
Non sono un servizio certificato di meteorologia spaziale: servono a capire \
cosa sta succedendo, non a prendere decisioni operative.",

            Self::Speed => "\
Velocità del plasma solare misurata al punto L1, in km/s. Il numero è l'ultimo \
minuto; il grafico copre le ultime 24 ore.\n\n\
Tipico: 300–500 km/s (vento lento). 500–800 km/s è vento veloce da buchi \
coronali; oltre 800 km/s quasi sempre l'arrivo di un'espulsione di massa \
coronale. Un salto brusco verso l'alto è il fronte di uno shock.\n\n\
Conta perché il vento veloce, se dura giorni, è ciò che **ricarica** la fascia \
esterna di elettroni relativistici.\n\n\
**Misura.**",

            Self::Density => "\
Numero di protoni per centimetro cubo nel vento solare, misurato a L1. Numero: \
ultimo minuto; grafico: ultime 24 ore.\n\n\
Tipico: 3–10 cm⁻³. Sopra 20 cm⁻³ il plasma è compresso (davanti a un flusso \
veloce o a uno shock). Sotto 1 cm⁻³ il vento è rarefatto, tipico dell'interno \
dei flussi veloci.\n\n\
Insieme alla velocità determina la pressione sulla magnetosfera.\n\n\
**Misura.**",

            Self::Temperature => "\
Temperatura dei protoni del vento solare, in kelvin, misurata a L1. Il grafico \
è in scala logaritmica.\n\n\
Tipico: da 10⁴ a qualche 10⁵ K, e cresce con la velocità. Vento veloce e caldo \
= flusso da buco coronale. Temperatura **insolitamente bassa** con campo \
magnetico forte e regolare è la firma di una nube magnetica, cioè il corpo di \
un'espulsione di massa coronale.\n\n\
**Misura.**",

            Self::Bt => "\
Intensità totale del campo magnetico interplanetario, in nanotesla, misurata a \
L1. È sempre positiva: dice quanto è forte il campo, non com'è orientato.\n\n\
Tipico: 3–8 nT. Sopra 10 nT il campo è rinforzato; sopra 20 nT c'è quasi \
sempre un'espulsione di massa coronale in transito.\n\n\
Bt fissa il limite di Bz: Bz non può scendere sotto −Bt. Con Bt alto, se il \
campo gira a sud, la tempesta può essere forte.\n\n\
**Misura.**",

            Self::Pressure => "\
Pressione dinamica del vento solare sulla magnetosfera, in nanopascal: \
P = 1,67·10⁻⁶ · n · V², con n in cm⁻³ e V in km/s. Numero: adesso; grafico: \
ultime 24 ore.\n\n\
Tipico: 1–3 nPa. Sopra 5 nPa (linea gialla) la magnetosfera è compressa; sopra \
10 nPa (linea rossa) la magnetopausa si avvicina all'orbita geostazionaria e \
la fascia esterna può perdere elettroni.\n\n\
Calcolo diretto da densità e velocità **misurate**.",

            Self::ImfSeries => "\
Le ultime 24 ore del campo magnetico interplanetario a L1. La curva arancione \
è **Bz** (componente nord-sud, sistema GSM), quella azzurra è **Bt** \
(intensità totale).\n\n\
Si guarda soprattutto Bz rispetto alla linea dello zero: quando resta \
**sotto zero** per ore, il campo solare si collega a quello terrestre e \
l'energia entra nella magnetosfera. La linea tratteggiata a −10 nT segna il \
livello oltre il quale, se dura, è probabile una tempesta.\n\n\
Bz può oscillare rapidamente fra nord e sud: contano la profondità e la \
durata, non il singolo minuto.\n\n\
**Misura.**",

            Self::ScaleR => "\
Scala NOAA dei blackout radio, da R0 (nessuno) a R5 (estremo). Dipende \
dall'intensità dei brillamenti solari in raggi X: R1 corrisponde a un \
brillamento di classe M1, R3 a un X1, R5 a un X20.\n\n\
Gli effetti riguardano le comunicazioni in onde corte sul lato illuminato \
della Terra. **Non riguarda le fasce di Van Allen**: è mostrata come contesto, \
perché un brillamento forte spesso precede di ore o giorni un disturbo \
geomagnetico.\n\n\
**Misura** (classificazione NOAA in corso).",

            Self::ScaleS => "\
Scala NOAA delle tempeste di radiazione solare, da S0 a S5. Dipende dal flusso \
di protoni sopra 10 MeV a orbita geostazionaria: S1 da 10 pfu, S2 da 100, S3 \
da 1000, S4 da 10 000, S5 da 100 000.\n\n\
Sono protoni accelerati dal Sole, non particelle delle fasce; ma un evento \
intenso può alimentare la fascia interna e disturba i sensori dei satelliti.\n\n\
**Misura** (classificazione NOAA in corso).",

            Self::ScaleG => "\
Scala NOAA delle tempeste geomagnetiche, da G0 a G5. Corrisponde al Kp: G1 con \
Kp 5, G2 con Kp 6, G3 con Kp 7, G4 con Kp 8, G5 con Kp 9.\n\n\
È il livello che più interessa le fasce: durante una tempesta la fascia \
esterna prima si svuota (dropout) e poi, spesso, si riempie più di prima.\n\n\
Qui è il valore ufficiale pubblicato da NOAA; nella scheda Fasce la scala G è \
ricavata dal Kp stimato al minuto, quindi per qualche minuto i due numeri \
possono differire.\n\n\
**Misura** (classificazione NOAA in corso).",

            Self::Hp => "\
Componente del campo magnetico parallela all'asse terrestre, in nanotesla, \
misurata dal magnetometro di GOES in orbita geostazionaria. Numero: ultimo \
minuto; grafico: ultime 6 ore.\n\n\
Tipico: 60–120 nT, con un'oscillazione giornaliera regolare (più alto a \
mezzogiorno locale del satellite, più basso a mezzanotte). Un aumento rapido \
indica compressione della magnetosfera; un campo molto basso o \
**negativo** significa che la magnetopausa ha superato il satellite, che si \
trova quindi fuori dalla magnetosfera.\n\n\
**Misura.**",

            Self::Protons => "\
Flusso integrale di protoni misurato da GOES a orbita geostazionaria, in pfu. \
Il numero è il canale sopra 10 MeV; il grafico mostra sopra 1, 10 e 100 MeV \
nelle ultime 6 ore, in scala logaritmica.\n\n\
In condizioni normali il canale > 10 MeV sta sotto 1 pfu. La linea \
tratteggiata a **10 pfu** è la soglia di tempesta di radiazione S1. Durante un \
evento le curve salgono di ordini di grandezza in poche ore e scendono in \
giorni.\n\n\
**Misura.**",

            Self::Plasmapause => "\
Distanza, in raggi terrestri, del bordo della plasmasfera: la nube di plasma \
freddo e denso che circonda la Terra. Dentro la plasmasfera le onde disperdono \
gli elettroni relativistici, quindi la plasmapausa segna il **bordo interno** \
della fascia esterna.\n\n\
Calcolata con la relazione di Carpenter e Anderson (1992), L = 5,6 − 0,46·Kp, \
limitata a 4,9 R⊕. In quiete sta attorno a 4,5–5 R⊕; durante una tempesta \
scende fino a 2–3 R⊕ e la fascia esterna la segue verso l'interno.\n\n\
**Modello**, non misura.",

            Self::KpSeries => "\
Kp stimato al minuto nelle ultime 6 ore. Ogni barra è un minuto; l'altezza è \
il valore di Kp e il colore la classe: verde fino a 4, giallo da 4, arancione \
da 5 (G1), rosso da 6 (G2), viola da 7 in su (G3–G5).\n\n\
La linea tratteggiata a Kp 5 è l'inizio della tempesta geomagnetica. Passando \
col puntatore si legge il valore del minuto.\n\n\
**Misura** (stima rapida NOAA dai magnetometri a terra).",

            Self::DstSeries => "\
Dst orario di Kyoto negli ultimi 7 giorni, in nanotesla. Una tempesta \
geomagnetica si riconosce dalla forma: breve salita iniziale (compressione), \
caduta rapida verso valori negativi (fase principale) e risalita lenta in uno \
o più giorni (recupero).\n\n\
Le linee tratteggiate a −50 e −100 nT, quando rientrano nel grafico, segnano \
tempesta moderata e forte.\n\n\
Nel sottotitolo c'è anche il **nowcast**: una stima al minuto del modello \
Geospace di NOAA, utile quando il dato orario non è ancora uscito.\n\n\
Serie: **misura** (provvisoria). Nowcast: **modello**.",

            Self::HistFlux => "\
Flusso di elettroni > 2 MeV a L = 6,6 registrato da questa app a ogni \
aggiornamento (uno ogni 5 minuti, solo mentre l'app è aperta), in scala \
logaritmica. La linea tratteggiata è la soglia di 1000 pfu.\n\n\
A differenza del grafico delle 24 ore, qui si vedono giorni e settimane: \
serve a riconoscere il ciclo tipico di una tempesta, con lo svuotamento \
iniziale e la ricarica nei giorni successivi.\n\n\
I buchi nella curva sono i periodi in cui l'app era chiusa; con lo storico \
remoto attivo (scheda Sorgenti) vengono riempiti da soli.\n\n\
**Misura** (campionata dall'app).",

            Self::HistKp => "\
Kp stimato registrato dall'app a ogni aggiornamento. Mostra quanto è stata \
disturbata la magnetosfera nel periodo scelto; la linea tratteggiata a Kp 5 è \
l'inizio della tempesta G1.\n\n\
Confrontalo con lo storico degli elettroni: di solito la fascia reagisce con \
uno o due giorni di ritardo rispetto al Kp.\n\n\
**Misura** (campionata dall'app).",

            Self::HistDst => "\
Dst registrato dall'app a ogni aggiornamento, in nanotesla. Più è negativo, \
più è intensa la corrente ad anello. Le linee tratteggiate a −50 e −100 nT, \
quando rientrano nel grafico, segnano tempesta moderata e forte.\n\n\
I punti registrati con versioni precedenti dell'app possono contenere il \
nowcast del modello invece del dato di Kyoto.\n\n\
**Misura** (campionata dall'app).",

            Self::HistMagnetopause => "\
Distanza della magnetopausa calcolata a ogni aggiornamento, in raggi \
terrestri. La linea tratteggiata a 6,6 R⊕ è l'orbita geostazionaria: quando \
la curva la raggiunge, la fascia esterna viene troncata.\n\n\
Segue la pressione del vento solare: ogni picco di pressione è un minimo di \
questa curva.\n\n\
**Modello** (formula di Shue), registrato nel tempo.",

            Self::HistBelt => "\
Posizione della fascia esterna nel tempo, in raggi terrestri: la curva verde \
è il picco (dove il flusso è massimo), quella gialla il bordo esterno (dove \
scende sotto 100 pfu).\n\n\
Durante una tempesta il picco si sposta verso la Terra e il bordo esterno si \
ritira; nei giorni di quiete la fascia si allarga di nuovo verso l'esterno.\n\n\
**Modello** ancorato alle misure, registrato nel tempo.",

            Self::HistContent => "\
Indice del contenuto totale della fascia esterna: l'integrale del profilo \
modellato, in unità arbitrarie, su scala logaritmica.\n\n\
Il numero in sé non ha un significato assoluto: va confrontato **con sé \
stesso** nel tempo. Se raddoppia, il modello stima il doppio di elettroni \
relativistici intrappolati.\n\n\
**Modello** ancorato alle misure, registrato nel tempo.",

            Self::Sources => "\
Una riga per ogni fonte di dati. Il pallino dice lo stato: **verde** dati \
recenti, **giallo** dati arrivati ma più vecchi del previsto, **rosso** \
errore di rete o risposta vuota.\n\n\
«Campioni» è quanti valori contiene la serie scaricata; «Ultimo dato» è \
l'ora UTC del campione più recente; «Età» è quanto tempo è passato da \
allora. Il Dst di Kyoto è orario, quindi un'età fino a un paio d'ore è \
normale; il nowcast Dst può avere età zero perché arriva fino a qualche \
decina di minuti nel futuro.\n\n\
Se una fonte è rossa, i riquadri che dipendono da essa mostrano «—» invece \
di un numero inventato. Tutte le fonti sono servizi pubblici NOAA SWPC, \
senza chiave.",

            Self::Server => "\
Mentre l'app è aperta, un piccolo server in sola lettura espone gli stessi \
numeri sul PC, all'indirizzo indicato: la pagina principale è una tabella che \
si aggiorna da sola ogni 30 secondi, **/snapshot.json** contiene i dati per \
script e altri programmi, **/health** risponde «ok» se l'app è viva.\n\n\
Ascolta solo su 127.0.0.1: non è raggiungibile dalla rete. Vive dentro l'app \
e si spegne chiudendo la finestra.\n\n\
Lo **storico** è il file indicato sotto: contiene un punto per ogni \
aggiornamento ed è ciò che alimenta la scheda Storico. Si può cancellare per \
ripartire da zero.\n\n\
Lo **storico remoto** è facoltativo: un piccolo servizio su Cloudflare che \
registra le stesse misure ogni 5 minuti anche quando l'app è chiusa. Se è \
configurato, l'app scarica i punti mancanti all'avvio e poi ogni mezz'ora, e \
ricalcola da quelle misure magnetopausa e forma delle fasce. La riga è verde \
se l'ultimo scaricamento è riuscito, rossa se è fallito.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Topic;

    #[test]
    fn every_topic_has_text() {
        for t in Topic::ALL {
            assert!(t.title().len() > 2, "{t:?} senza titolo");
            assert!(t.body().len() > 150, "{t:?} con testo troppo corto");
            // i grassetti devono essere chiusi
            assert_eq!(t.body().matches("**").count() % 2, 0, "{t:?}: ** spaiati");
        }
    }
}
