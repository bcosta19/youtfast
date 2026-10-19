# Diagnóstico do carregamento indefinido de For you — 2026-10-16

Foi reproduzido um mecanismo capaz de produzir o sintoma relatado: o pedido
principal de recomendações termina com `panic`, nenhuma resposta chega à
interface e “Loading recommendations…” continua desenhado. Uma solicitação
independente no mesmo backend continua respondendo.

A reprodução confirma o defeito, mas não prova que essa foi a ocorrência exata
na instância reiniciada. O processo anterior já terminou e não havia captura
persistente de panics. Os logs preservados contêm categorias genéricas, sem
informação suficiente para recuperar a resposta ou exceção original.

## Como o defeito acontece

1. A rota de recomendações da conta chama `innertube::recommended` e o pedido
   `raw` da RustyPipe. Antes de enviar o pedido, a biblioteca obtém seus dados
   técnicos de visitante quando ainda não os tem em memória.
2. `vendor/rustypipe/src/util/visitor_data.rs:113` usa
   `self.fetch_visitor_data().await.unwrap()`. Uma falha de transporte,
   timeout ou resposta da qual não se consiga extrair esses dados pode provocar
   `panic`, em vez de retornar `Err`.
3. `src/backend.rs:643` executa o carregamento em `tokio::spawn`. O evento
   `Event::Loaded` só é enviado depois que `load(...).await` retorna normalmente.
   O resultado final desse `JoinHandle` não é supervisionado; se houver panic,
   o evento não é enviado.
4. `src/app/loading.rs` mantém a lista em `Listing::Loading`. O carregamento
   não tem um prazo total e a navegação comum só refaz estados `Idle` ou
   `Failed`. Portanto, o estado preso pode continuar até uma atualização
   explícita ou reinício.

Isso explica por que somente For you pode ficar preso enquanto a busca
funciona: a tarefa que falhou já terminou; o backend e as consultas posteriores
podem continuar operando. Ao reiniciar, a lista começa de novo e a obtenção dos
dados técnicos pode funcionar na nova tentativa. Uma falha transitória nesse
primeiro pedido é uma hipótese consistente com o episódio relatado; o gatilho
histórico específico não foi confirmado.

## Reprodução controlada

Foi usada uma cópia descartável dos fontes atuais, com o backend e a interface
egui reais. A tarefa foi iniciada pelo mesmo `handle(Command::Load, Context)`
da aplicação. O estado visual foi verificado nas formas de texto produzidas
por `ui::show`, em egui sem janela nativa.

Um proxy local em `127.0.0.1` rejeitou o túnel HTTPS com 502, provocando uma
falha de transporte antes do pedido de recomendações. Não houve contato com
os servidores do Google nesses testes. A conta, a lista e os dados de visitante
eram sintéticos; cookies, chaveiro, biblioteca, histórico e cache do usuário
não foram lidos. O armazenamento usado pelos clientes era desativado ou uma
fixture pública em memória.

| Cenário | Resultado observado |
| --- | --- |
| Recomendações da conta, sem dados de visitante em memória, falha de transporte | Panic em `visitor_data.rs:113`; tarefa terminou; nenhum `Event::Loaded`; texto de carregamento permaneceu visível. |
| Solicitação independente no mesmo backend depois da falha | Evento de conclusão recebido normalmente. O controle usou uma lista sintética vazia, sem rede. |
| Recomendações locais com falha na tarefa auxiliar | O `JoinSet` capturou a falha do filho e o carregamento principal enviou um evento de conclusão. |
| Dados técnicos em memória e falha tratável de autenticação sintética | Sem panic; o caminho alternativo concluiu e enviou evento. |

O protocolo completo de quatro cenários passou em três execuções consecutivas,
com um núcleo, um job de compilação e prioridade baixa. Foi testado o perfil
`debug` usado pelo executável atual, em Linux. Os cenários não foram executados
na conta real nem em uma segunda janela do aplicativo.

Saída controlada preservada das três execuções:

```text
controlled_panic_location=visitor_data.rs:113
CASE account_recommendations_transport_failure: task_panicked=true completion_event=false loading_visible=true
CASE same_backend_control: independent_completion=true
controlled_panic_location=visitor_data.rs:113
CASE local_fallback_transport_failure: child_panicked=true completion_event=true
CASE ordinary_authentication_failure: task_panicked=false completion_event=true
LOG_CLASSIFICATION error_level_span_records=1 other_error_records=0
test result: ok. 1 passed; 0 failed; 153 filtered out
```

## O que os logs permitem concluir

As duas sessões, antes e depois do reinício, registraram linhas genéricas no
nível `ERROR`. No teste foi identificado um registro nesse nível originado da
instrumentação de `video_details`, e nenhum outro registro de erro. A RustyPipe
anota essa operação com `#[tracing::instrument(..., level = "error")]`, que
pode gerar uma linha no log quando a operação é instrumentada. A categoria
atual não distingue essas linhas de um erro efetivo.

Portanto, a quantidade dessas linhas não demonstra que a rede falhou ou que
elas causaram o travamento. Além disso, a configuração atual não instala um
hook persistente de panic, e a instância investigada tinha stderr em
`/dev/null`. A informação necessária para confirmar retrospectivamente o panic
original não está disponível.

## Correção indicada, ainda não aplicada

- Propagar a falha de obtenção de dados de visitante com `?`, preservando o
  contrato `Result`, em vez de usar `unwrap()`.
- Supervisionar a conclusão dos carregamentos e impor um prazo total. Falhas
  internas e timeout devem produzir eventos com mensagens controladas, mantendo
  as proteções de geração e sessão e o cancelamento de pedidos substituídos.
- Registrar início, fim e categoria de falha por etapa, incluindo panics com
  payload omitido. Não registrar URLs, tokens, cookies, respostas ou mensagens
  brutas de dependências. Diferenciar instrumentação de erro efetivo.

Os arquivos de produção e o executável permaneceram inalterados nesta
investigação. A instância aberta não foi fechada nem reiniciada.
