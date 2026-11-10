exit: 1
--- stdout
--- stderr
Problem on line 3 of block.ag: there is no parameter called `a2a_leitor`
  3 |   remote leitor at @parameters.a2a_leitor
    |                    ^
Fix: add it under [parameters] in metagente.toml, for example: a2a_leitor = "some text"
