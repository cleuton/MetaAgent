exit: 1
--- stdout
--- stderr
Problem on line 5 of <tmp>/metagente.toml: I could not read the certificate file <tmp>/certs/dev-ca.pem: No such file or directory (os error 2)
Fix: check the path of ca_file (a relative path starts at the folder of metagente.toml)

Found 1 problem.
