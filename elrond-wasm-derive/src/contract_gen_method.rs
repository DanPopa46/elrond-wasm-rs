use super::arg_def::*;
use super::arg_extract::*;
use super::arg_regular::*;
use super::contract_gen_finish::*;
use super::contract_gen_payable::*;
use super::parse_attr::*;
use super::reserved;
use super::util::*;

/// Method visibility from the point of view of the smart contract
#[derive(Clone, Debug)]
pub enum Visibility {
	/// Means it gets a smart contract function generated for it
	Endpoint(syn::Ident),

	/// Can be used only inside the smart contract, even if it is public in the module.
	Private,
}

/// Contains metdata from the `#[payable]` attribute.
#[derive(Clone, Debug)]
pub enum MethodPayableMetadata {
	NoMetadata,
	NotPayable,
	Egld,
	SingleEsdtToken(String),
	AnyToken,
}

impl MethodPayableMetadata {
	pub fn is_payable(&self) -> bool {
		!matches!(self, MethodPayableMetadata::NotPayable)
	}

	pub fn abi_strings(&self) -> Vec<String> {
		match self {
			MethodPayableMetadata::NoMetadata | MethodPayableMetadata::NotPayable => Vec::new(),
			MethodPayableMetadata::Egld => vec!["EGLD".to_string()],
			MethodPayableMetadata::SingleEsdtToken(s) => vec![s.clone()],
			MethodPayableMetadata::AnyToken => vec!["*".to_string()],
		}
	}
}

pub fn process_payable(m: &syn::TraitItemMethod) -> MethodPayableMetadata {
	let payable_attr_opt = PayableAttribute::parse(m);
	if let Some(payable_attr) = payable_attr_opt {
		if let Some(identifier) = payable_attr.identifier {
			match identifier.as_str() {
				"EGLD" => MethodPayableMetadata::Egld,
				"*" => MethodPayableMetadata::AnyToken,
				"" => panic!("empty token name not allowed in #[payable] attribute"),
				_ => MethodPayableMetadata::SingleEsdtToken(identifier),
			}
		} else {
			eprintln!("Warning: usage of #[payable] without argument is deprecated. Replace with #[payable(\"EGLD\")]. Method name: {}", m.sig.ident.to_string());
			MethodPayableMetadata::Egld
		}
	} else {
		MethodPayableMetadata::NotPayable
	}
}

#[derive(Clone, Debug)]
pub enum MethodMetadata {
	Regular {
		visibility: Visibility,
		payable: MethodPayableMetadata,
	},
	LegacyEvent {
		identifier: Vec<u8>,
	},
	Event {
		identifier: String,
	},
	Callback,
	CallbackRaw,
	StorageGetter {
		visibility: Visibility,
		identifier: String,
	},
	StorageSetter {
		visibility: Visibility,
		identifier: String,
	},
	StorageMapper {
		visibility: Visibility,
		identifier: String,
	},
	StorageGetMut {
		visibility: Visibility,
		identifier: String,
	},
	StorageIsEmpty {
		visibility: Visibility,
		identifier: String,
	},
	StorageClear {
		visibility: Visibility,
		identifier: String,
	},
	Module {
		impl_path: proc_macro2::TokenTree,
	},
}

impl MethodMetadata {
	pub fn endpoint_name(&self) -> Option<&syn::Ident> {
		match self {
			MethodMetadata::Regular {
				visibility: Visibility::Endpoint(e),
				..
			}
			| MethodMetadata::StorageGetter {
				visibility: Visibility::Endpoint(e),
				..
			}
			| MethodMetadata::StorageSetter {
				visibility: Visibility::Endpoint(e),
				..
			}
			| MethodMetadata::StorageMapper {
				visibility: Visibility::Endpoint(e),
				..
			}
			| MethodMetadata::StorageGetMut {
				visibility: Visibility::Endpoint(e),
				..
			}
			| MethodMetadata::StorageIsEmpty {
				visibility: Visibility::Endpoint(e),
				..
			}
			| MethodMetadata::StorageClear {
				visibility: Visibility::Endpoint(e),
				..
			} => Some(e),
			_ => None,
		}
	}

	pub fn has_implementation(&self) -> bool {
		matches!(
			self,
			MethodMetadata::Regular { .. } | MethodMetadata::Callback | MethodMetadata::CallbackRaw
		)
	}

	pub fn payable_metadata(&self) -> MethodPayableMetadata {
		match self {
			MethodMetadata::Regular { payable, .. } => payable.clone(),
			MethodMetadata::Callback | MethodMetadata::CallbackRaw => {
				MethodPayableMetadata::AnyToken
			},
			MethodMetadata::StorageGetter { .. } => MethodPayableMetadata::NotPayable,
			MethodMetadata::StorageSetter { .. } => MethodPayableMetadata::NotPayable,
			MethodMetadata::StorageGetMut { .. } => MethodPayableMetadata::NotPayable,
			MethodMetadata::StorageIsEmpty { .. } => MethodPayableMetadata::NotPayable,
			MethodMetadata::StorageClear { .. } => MethodPayableMetadata::NotPayable,
			_ => MethodPayableMetadata::NoMetadata,
		}
	}
}

#[derive(Clone, Debug)]
pub struct Method {
	pub docs: Vec<String>,
	pub metadata: MethodMetadata,
	pub name: syn::Ident,
	pub generics: syn::Generics,
	pub method_args: Vec<MethodArg>,
	pub payment_arg: Option<MethodArg>,
	pub token_arg: Option<MethodArg>,
	pub output_names: Vec<String>,
	pub return_type: syn::ReturnType,
	pub body: Option<syn::Block>,
}

const INIT_ENDPOINT_NAME: &str = "init";

fn process_visibility(m: &syn::TraitItemMethod) -> Visibility {
	let endpoint_attr_opt = EndpointAttribute::parse(m);
	let view_attr_opt = ViewAttribute::parse(m);

	// init
	let init = is_init(m);
	if init {
		if endpoint_attr_opt.is_some() {
			panic!("Cannot annotate with both #[init] and #[endpoint].");
		}
		if view_attr_opt.is_some() {
			panic!("Cannot annotate with both #[init] and #[view].");
		}
		return Visibility::Endpoint(syn::Ident::new(INIT_ENDPOINT_NAME, m.sig.ident.span()));
	}

	// endpoint
	if let Some(endpoint_attr) = endpoint_attr_opt {
		if view_attr_opt.is_some() {
			panic!("Cannot annotate with both #[endpoint] and #[view].");
		}
		let endpoint_ident = match endpoint_attr.endpoint_name {
			Some(ident) => ident,
			None => m.sig.ident.clone(),
		};
		let endpoint_name_str = &endpoint_ident.to_string();
		if endpoint_name_str == INIT_ENDPOINT_NAME {
			panic!("Cannot declare endpoint with name 'init'. Use #[init] instead.")
		}
		if reserved::is_reserved(endpoint_name_str) {
			panic!("Cannot declare endpoint with name '{}', because that name is reserved by the Arwen API.", endpoint_name_str);
		}
		return Visibility::Endpoint(endpoint_ident);
	}

	// view
	if let Some(view_attr) = view_attr_opt {
		let view_ident = match view_attr.view_name {
			Some(ident) => ident,
			None => m.sig.ident.clone(),
		};
		let view_name_str = &view_ident.to_string();
		if view_name_str == INIT_ENDPOINT_NAME {
			panic!("Cannot declare view with name 'init'. Use #[init] instead.")
		}
		if reserved::is_reserved(view_name_str) {
			panic!("Cannot declare view with name '{}', because that name is reserved by the Arwen API.", view_name_str);
		}
		return Visibility::Endpoint(view_ident);
	}

	Visibility::Private
}

fn extract_metadata(m: &syn::TraitItemMethod) -> MethodMetadata {
	let visibility = process_visibility(m);
	let payable = process_payable(m);
	let callback = is_callback_decl(m);
	let callback_raw = is_callback_raw_decl(m);
	let legacy_event_opt = LegacyEventAttribute::parse(m);
	let event_opt = EventAttribute::parse(m);
	let storage_get_opt = StorageGetAttribute::parse(m);
	let storage_set_opt = StorageSetAttribute::parse(m);
	let storage_mapper_opt = StorageMapperAttribute::parse(m);
	let storage_get_mut_opt = StorageGetMutAttribute::parse(m);
	let storage_is_empty_opt = StorageIsEmptyAttribute::parse(m);
	let storage_clear_opt = StorageClearAttribute::parse(m);
	let module_opt = ModuleAttribute::parse(m);

	// TODO: this entire component requires extensive refactoring
	if let Some(event_attr) = legacy_event_opt {
		if payable.is_payable() {
			panic!("Events cannot be payable.");
		}
		if let Visibility::Endpoint(_) = visibility {
			panic!("Events cannot be endpoints.");
		}
		if callback || callback_raw {
			panic!("Events cannot be callbacks.");
		}
		if storage_get_opt.is_some() {
			panic!("Events cannot be storage getters.");
		}
		if storage_set_opt.is_some() {
			panic!("Events cannot be storage setters.");
		}
		if storage_mapper_opt.is_some() {
			panic!("Events cannot be storage mappers.");
		}
		if storage_get_mut_opt.is_some() {
			panic!("Events cannot be storage borrow getters.");
		}
		if module_opt.is_some() {
			panic!("Events cannot be modules.");
		}
		if m.default.is_some() {
			panic!("Events cannot have an implementations provided in the trait.");
		}
		MethodMetadata::LegacyEvent {
			identifier: event_attr.identifier,
		}
	} else if let Some(event_attr) = event_opt {
		if payable.is_payable() {
			panic!("Events cannot be payable.");
		}
		if let Visibility::Endpoint(_) = visibility {
			panic!("Events cannot be endpoints.");
		}
		if callback || callback_raw {
			panic!("Events cannot be callbacks.");
		}
		if storage_get_opt.is_some() {
			panic!("Events cannot be storage getters.");
		}
		if storage_set_opt.is_some() {
			panic!("Events cannot be storage setters.");
		}
		if storage_mapper_opt.is_some() {
			panic!("Events cannot be storage mappers.");
		}
		if storage_get_mut_opt.is_some() {
			panic!("Events cannot be storage borrow getters.");
		}
		if module_opt.is_some() {
			panic!("Events cannot be modules.");
		}
		if m.default.is_some() {
			panic!("Events cannot have an implementations provided in the trait.");
		}
		MethodMetadata::Event {
			identifier: event_attr.identifier,
		}
	} else if callback || callback_raw {
		if payable.is_payable() {
			panic!("Callback methods cannot be marked payable.");
		}
		if let Visibility::Endpoint(_) = visibility {
			panic!("Callbacks cannot be endpoints.");
		}
		if storage_get_opt.is_some() {
			panic!("Callbacks cannot be storage getters.");
		}
		if storage_set_opt.is_some() {
			panic!("Callbacks cannot be storage setters.");
		}
		if storage_mapper_opt.is_some() {
			panic!("Callbacks cannot be storage mappers.");
		}
		if storage_get_mut_opt.is_some() {
			panic!("Callbacks cannot be storage borrow getters.");
		}
		if module_opt.is_some() {
			panic!("Callbacks cannot be modules.");
		}
		if m.default.is_none() {
			panic!("Callback methods need an implementation.");
		}
		if callback && callback_raw {
			panic!("It is either the default callback, or regular callback, not both.");
		}
		if callback_raw {
			MethodMetadata::CallbackRaw
		} else {
			MethodMetadata::Callback
		}
	} else if let Some(storage_get) = storage_get_opt {
		if payable.is_payable() {
			panic!("Storage getters cannot be marked payable.");
		}
		if m.default.is_some() {
			panic!("Storage getters cannot have an implementations provided in the trait.");
		}
		if module_opt.is_some() {
			panic!("Storage getters cannot be modules.");
		}
		MethodMetadata::StorageGetter {
			visibility,
			identifier: storage_get.identifier,
		}
	} else if let Some(storage_set) = storage_set_opt {
		if payable.is_payable() {
			panic!("Storage setters cannot be marked payable.");
		}
		if m.default.is_some() {
			panic!("Storage setters cannot have an implementations provided in the trait.");
		}
		if module_opt.is_some() {
			panic!("Storage setters cannot be modules.");
		}
		MethodMetadata::StorageSetter {
			visibility,
			identifier: storage_set.identifier,
		}
	} else if let Some(storage_mapper) = storage_mapper_opt {
		if payable.is_payable() {
			panic!("Storage mappers cannot be marked payable.");
		}
		if m.default.is_some() {
			panic!("Storage mappers cannot have an implementations provided in the trait.");
		}
		if module_opt.is_some() {
			panic!("Storage mappers cannot be modules.");
		}
		MethodMetadata::StorageMapper {
			visibility,
			identifier: storage_mapper.identifier,
		}
	} else if let Some(storage_get_mut) = storage_get_mut_opt {
		if payable.is_payable() {
			panic!("Storage mutable getters cannot be marked payable.");
		}
		if m.default.is_some() {
			panic!("Storage mutable getters cannot have an implementations provided in the trait.");
		}
		if module_opt.is_some() {
			panic!("Storage mutable getters cannot be modules.");
		}
		MethodMetadata::StorageGetMut {
			visibility,
			identifier: storage_get_mut.identifier,
		}
	} else if let Some(storage_is_empty) = storage_is_empty_opt {
		if payable.is_payable() {
			panic!("Storage is empty cannot be marked payable.");
		}
		if m.default.is_some() {
			panic!("Storage is empty cannot have an implementations provided in the trait.");
		}
		if module_opt.is_some() {
			panic!("Storage is empty cannot be modules.");
		}
		MethodMetadata::StorageIsEmpty {
			visibility,
			identifier: storage_is_empty.identifier,
		}
	} else if let Some(storage_clear) = storage_clear_opt {
		if payable.is_payable() {
			panic!("Storage clear cannot be marked payable.");
		}
		if m.default.is_some() {
			panic!("Storage clear cannot have an implementations provided in the trait.");
		}
		if module_opt.is_some() {
			panic!("Storage clear cannot be modules.");
		}
		MethodMetadata::StorageClear {
			visibility,
			identifier: storage_clear.identifier,
		}
	} else if let Some(module_attr) = module_opt {
		if m.default.is_some() {
			panic!("Module declarations cannot have an implementations provided in the trait.");
		}
		MethodMetadata::Module {
			impl_path: module_attr.arg,
		}
	} else {
		if m.default.is_none() {
			panic!(
				"Regular methods need an implementation: {}",
				m.sig.ident.to_string()
			);
		}
		MethodMetadata::Regular {
			visibility,
			payable: payable,
		}
	}
}

impl Method {
	pub fn parse(m: &syn::TraitItemMethod) -> Method {
		let metadata = extract_metadata(m);
		let method_args = extract_method_args(m);
		let payment_arg = extract_payment(metadata.payable_metadata(), &method_args[..]);
		let token_arg = extract_payment_token(metadata.payable_metadata(), &method_args[..]);
		let output_names = find_output_names(m);
		Method {
			docs: extract_doc(m.attrs.as_slice()),
			metadata,
			name: m.sig.ident.clone(),
			generics: m.sig.generics.clone(),
			method_args,
			payment_arg,
			token_arg,
			output_names,
			return_type: m.sig.output.clone(),
			body: m.default.clone(),
		}
	}
}

pub fn arg_declarations(method_args: &[MethodArg]) -> Vec<proc_macro2::TokenStream> {
	method_args
		.iter()
		.map(|arg| {
			let pat = &arg.pat;
			let ty = &arg.ty;
			quote! {#pat : #ty }
		})
		.collect()
}

impl Method {
	pub fn generate_sig(&self) -> proc_macro2::TokenStream {
		let method_name = &self.name;
		let generics = &self.generics;
		let generics_where = &self.generics.where_clause;
		let arg_decl = arg_declarations(&self.method_args);
		let ret_tok = match &self.return_type {
			syn::ReturnType::Default => quote! {},
			syn::ReturnType::Type(_, ty) => quote! { -> #ty },
		};
		let result = quote! { fn #method_name #generics ( &self , #(#arg_decl),* ) #ret_tok #generics_where };
		result
	}

	pub fn generate_call_to_method(&self) -> proc_macro2::TokenStream {
		let fn_ident = &self.name;
		let arg_values: Vec<proc_macro2::TokenStream> = self
			.method_args
			.iter()
			.map(|arg| generate_arg_call_name(arg))
			.collect();
		quote! {
			self.#fn_ident (#(#arg_values),*)
		}
	}

	pub fn has_variable_nr_args(&self) -> bool {
		self.method_args
			.iter()
			.any(|arg| matches!(&arg.metadata, ArgMetadata::VarArgs))
	}

	pub fn generate_call_method(&self) -> proc_macro2::TokenStream {
		let call_method_ident = generate_call_method_name(&self.name);
		let call_method_body = self.generate_call_method_body();
		quote! {
			#[inline]
			fn #call_method_ident (&self) {
				#call_method_body
			}
		}
	}

	pub fn generate_call_method_body(&self) -> proc_macro2::TokenStream {
		if self.has_variable_nr_args() {
			self.generate_call_method_body_variable_nr_args()
		} else {
			self.generate_call_method_body_fixed_args()
		}
	}

	pub fn generate_call_method_body_fixed_args(&self) -> proc_macro2::TokenStream {
		let payable_snippet = generate_payable_snippet(self);

		let mut arg_index = -1i32;
		let arg_init_snippets: Vec<proc_macro2::TokenStream> = self
			.method_args
			.iter()
			.map(|arg| match &arg.metadata {
				ArgMetadata::Single => {
					arg_index += 1;
					let pat = &arg.pat;
					let arg_get = generate_load_single_arg(arg, &quote! { #arg_index });
					quote! {
						let #pat = #arg_get;
					}
				},
				ArgMetadata::Payment | ArgMetadata::PaymentToken => quote! {},
				ArgMetadata::VarArgs => {
					panic!("var_args not accepted in function generate_call_method_fixed_args")
				},
				ArgMetadata::AsyncCallResultArg => {
					panic!("async call result arg not allowed here")
				},
			})
			.collect();

		let call = self.generate_call_to_method();
		let body_with_result = generate_body_with_result(&self.return_type, &call);
		let nr_args = arg_index + 1;

		quote! {
			#payable_snippet
			self.api.check_num_arguments(#nr_args);
			#(#arg_init_snippets)*
			#body_with_result
		}
	}

	fn generate_call_method_body_variable_nr_args(&self) -> proc_macro2::TokenStream {
		let payable_snippet = generate_payable_snippet(self);

		let arg_init_snippets: Vec<proc_macro2::TokenStream> = self
			.method_args
			.iter()
			.map(|arg| match &arg.metadata {
				ArgMetadata::Single | ArgMetadata::VarArgs => {
					generate_load_dyn_arg(arg, &quote! { &mut ___arg_loader })
				},
				ArgMetadata::Payment | ArgMetadata::PaymentToken => quote! {},
				ArgMetadata::AsyncCallResultArg => panic!("async call result arg npt allowed here"),
			})
			.collect();

		let call = self.generate_call_to_method();
		let body_with_result = generate_body_with_result(&self.return_type, &call);

		quote! {
			#payable_snippet

			let mut ___arg_loader = EndpointDynArgLoader::new(self.api.clone());

			#(#arg_init_snippets)*

			___arg_loader.assert_no_more_args();

			#body_with_result
		}
	}
}
