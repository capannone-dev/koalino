<!-- Reading scripts for recording real multi-speaker test audio, used to check diarization and voice naming (#61, #63). -->

# Copioni di prova

Un copione per ogni combinazione e ordine di Ettore, Renzo e Marta: 15 in tutto, ciascuno di circa un minuto e mezzo.

| Persone | Copioni | Chi parla per primo |
| --- | --- | --- |
| Una | `01`–`03` | la persona indicata |
| Due | `04`–`09` | ogni coppia, in entrambi gli ordini |
| Tre | `10`–`15` | tutti i 6 ordini |

## Come registrare

1. L'ordine dei nomi nel file è l'ordine in cui le persone parlano per la prima volta.
2. Ogni riga `NOME: testo` è una battuta. Leggete solo il testo, con voce naturale e senza sovrapporvi.
3. Salvate la registrazione qui con lo stesso nome e l'estensione audio: `04_ettore_renzo.txt` diventa
   `04_ettore_renzo.wav` (o `.m4a`, `.mp3`).
4. Le prime due righe del file e le righe vuote non vanno lette.

## Privacy

Le registrazioni contengono voci, cioè dati biometrici. Il `.gitignore` di questa cartella tiene fuori
da git tutto tranne copioni e README, quindi l'audio resta solo su questo computer. Chiedete il consenso
a chi legge, e cancellate i file quando i test sono finiti.
