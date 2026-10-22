# As mensagens do MK Perth em português do Brasil. Ids de comandos e formatos de arquivo nunca se traduzem.

err-calendar-no-working-days = O calendário { $name } não tem dia útil para posicionar uma atividade.
err-calendar-out-of-range = O calendário { $name } passou da última data que consegue contar.
err-duplicate-activity = Duas atividades têm o ID { $id }; cada ID precisa ser único.
err-unknown-activity = Um vínculo cita a atividade { $id }, que não está no projeto.
err-self-relationship = A atividade { $id } está vinculada a si mesma.
err-negative-duration = A atividade { $id } tem duração negativa.
err-unknown-calendar = A atividade { $id } trabalha no calendário { $name }, que o projeto não tem.
err-unknown-parent = A atividade { $id } está sob { $parent }, que não está no projeto.
err-parent-loop = Estas atividades estão uma sob a outra em ciclo: { $ids }
err-milestone-parent = O marco { $id } tem atividades sob ele; um marco não pode ser resumo.
err-loop = Estes vínculos formam um ciclo, então nenhuma ordem os satisfaz: { $ids }
err-too-many = O projeto tem mais { $what } do que os { $max } que o MK Perth agenda.
err-too-long = O cronograma passa de { $max } dias úteis, o mais longo que o MK Perth posiciona.
what-activities = atividades
what-relationships = vínculos

err-perth-json = Isto não é um JSON que o Perth gravou: { $detail }
err-perth-not-a-project = Este JSON não é um projeto do Perth: não tem lista de tarefas.
err-perth-field = Tarefa { $task }: o campo { $field } não é o que o Perth grava ali.
err-perth-too-many = O arquivo tem mais do que o MK Perth lê (no máximo { $max } tarefas, e outras tantas dependências por tarefa).
warn-perth-duration = A tarefa { $task } tinha duração menor que um dia; o Perth conta como um.
warn-perth-dangling = A tarefa { $task } depende de { $dependency }, que não está no projeto; ficou de fora, como o Perth faz.
warn-perth-constraint = A tarefa { $task } tem uma restrição que o Perth não tem como guardar; ela não é gravada.
warn-perth-sf = A tarefa { $task } segue { $predecessor } de início a fim, vínculo que o Perth não tem; ele não é gravado.
