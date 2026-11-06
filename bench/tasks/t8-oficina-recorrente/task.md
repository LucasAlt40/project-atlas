Precisamos ajustar a área de pagamentos do backend (Java, Spring Boot). O seguinte problema está acontecendo: ao fazer uma doação recorrente (uma assinatura no PagBank), o registro do pagamento é feito, mas o mês seguinte remove o do mês anterior. Quando a pessoa é cobrada em agosto, o pagamento é registrado normalmente; no mês seguinte, em setembro, o de setembro é registrado e o de agosto some da listagem de doações.

O PagBank avisa cada cobrança pelo webhook POST /api/notifications/payment, e todas as cobranças da mesma assinatura chegam com o mesmo reference_id (o id da doação). Encontre a causa e corrija. O comportamento esperado:

1. Cada cobrança mensal de uma doação recorrente fica registrada como um pagamento próprio (tabela payment): a cobrança de setembro não pode alterar a data nem o status do pagamento de agosto, inclusive quando a de setembro é recusada.
2. A primeira cobrança de uma doação (o pagamento criado no checkout, ainda não pago) continua atualizando esse pagamento, sem criar outro.
3. Uma notificação repetida no mesmo mês, para uma doação que já tem um pagamento pago naquele mês, não cria outro pagamento.
4. Uma doação única (não recorrente) nunca ganha um segundo pagamento.
5. O resto do fluxo continua como está: método, bandeira, taxa e ativação do apadrinhamento.

Não altere o esquema do banco de dados. Adicione testes de regressão em src/test/java com JUnit 5 e Mockito (spring-boot-starter-test já está no projeto). Os testes rodam com `mvn test`; o teste OficinadasmeninasApplicationTests precisa de um banco e falha sem ele, então rode só os seus (por exemplo `mvn test -Dtest=NomeDoSeuTeste`).
