# Things to think about
- Should I have a global config so every contract share the same data? 
- What happens when an user is banned but there is a booking still active? The money should be returned to the client or Stayke should take a part of it?


# Missing things
TODO: enforce security. We don't allow modifications from other contracts unless we secure them beforehand and I think the best way to handle this all is by creating a global contract. It is the easiest way and we can share the config through all the files easily. We also need to have config accounts in each contract that are a reference for this global contract.

Why should we have this contract instead of another? I don't really want to over-complicate much everything, so I guess having a global variable is a wiser way. 
