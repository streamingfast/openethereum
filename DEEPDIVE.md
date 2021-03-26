# Instruction Execution

- `Exec` this is the Virtual machine either wasm or EVM, it exposes a function `.exec(...)` that will attempt to run through the call
s OPCODE. The call site of `.exec` is the common entry point for either `wasm` and `evm` vm (`ethcore/vm/src/lib.rs`)

-> `Executive.call_with_stack_depth`: (`ethcore/machine/src/executive.rs:1027`)
  	* Calls contract function with given contract params and stack depth
	-> `CallCreateExecutive.consume`: (`ethcore/machine/src/executive.rs:661`)
		* execute the current call, this will loop and continue until the call is completed
		-> `CallCreateExecutive.exec`: (`ethcore/machine/src/executive.rs:391`)
  			* Execute the call. If a sub-call/create action is required, a resume trap error is returned. The caller (`consume`) is then
			expected to call `resume_call` or `resume_create` to continue the execution. It will handle the different Call type:
				- `CallCreateExecutiveKind::Transfer`
				- `CallCreateExecutiveKind::CallBuiltin`
	  				* These are calls that are already builtin into the code at specific address, the list of built in
					calls can be found here `ethcore/builtin/src/lib.rs:348`.
				- `CallCreateExecutiveKind::ExecCall`
	  				* Is a vm call that will be executed with `vm.exec` HURRAY! - 1
				- `CallCreateExecutiveKind::ExecCreate`
	  				* Is a vm create call,with `vm.exec` HURRAY! - 2
	  			- `CallCreateExecutiveKind::ResumeCall`
	  				* PANIC! these types will be handled by other function `CallCreateExecutiv.resume_call`
	  			- `CallCreateExecutiveKind::ResumeCreate`
					* PANIC! these types will be handled by other function `CallCreateExecutiv.resume_create`
  		-> `CallCreateExecutive.resume_call`: (`ethcore/machine/src/executive.rs:585`)
			* Resumes a call with  `vm.exec` HURRAY! - 3
  		-> `CallCreateExecutive.resume_create`: (`ethcore/machine/src/executive.rs:504 & 631`)
  			* Resumes a create call with  `vm.exec` HURRAY! - 4



