use proc_macro2;
use proc_macro2::Span;
use syn;

use field::*;
use model::*;
use util::*;

pub fn derive(item: syn::DeriveInput) -> Result<proc_macro2::TokenStream, Diagnostic> {
    let model = Model::from_item(&item)?;

    if model.fields().is_empty() {
        return Err(Span::call_site()
            .error("Cannot derive Insertable for unit structs")
            .help(format!(
                "Use `insert_into({}::table).default_values()` if you want `DEFAULT VALUES`",
                model.table_name()
            )));
    }

    let table_name = &model.table_name();
    let struct_name = &item.ident;

    let (_, ty_generics, where_clause) = item.generics.split_for_impl();
    let mut impl_generics = item.generics.clone();
    impl_generics.params.push(parse_quote!('insert));
    let (impl_generics, ..) = impl_generics.split_for_impl();

    let mut generate_borrowed_insert = true;

    let mut direct_field_ty = Vec::with_capacity(model.fields().len());
    let mut direct_field_assign = Vec::with_capacity(model.fields().len());
    let mut ref_field_ty = Vec::with_capacity(model.fields().len());
    let mut ref_field_assign = Vec::with_capacity(model.fields().len());

    for field in model.fields() {
        let serialize_as = field.ty_for_serialize()?;
        let embed = field.has_flag("embed");

        match (serialize_as, embed) {
            (None, true) => {
                direct_field_ty.push(field_ty_embed(field, None));
                direct_field_assign.push(field_expr_embed(field, None));
                ref_field_ty.push(field_ty_embed(field, Some(quote!(&'insert))));
                ref_field_assign.push(field_expr_embed(field, Some(quote!(&))));
            }
            (None, false) => {
                direct_field_ty.push(field_ty(field, table_name, None));
                direct_field_assign.push(field_expr(field, table_name, None));
                ref_field_ty.push(field_ty(field, table_name, Some(quote!(&'insert))));
                ref_field_assign.push(field_expr(field, table_name, Some(quote!(&))));
            }
            (Some(ty), false) => {
                direct_field_ty.push(field_ty_serialize_as(field, table_name, &ty));
                direct_field_assign.push(field_expr_serialize_as(field, table_name, &ty));

                generate_borrowed_insert = false; // as soon as we hit one field with #[diesel(serialize_as)] there is no point in generating the impl of Insertable for borrowed structs
            }
            (Some(_), true) => {
                return Err(field
                    .flags
                    .span()
                    .error("`#[diesel(embed)]` cannot be combined with `#[diesel(serialize_as)]`"))
            }
        }
    }

    let insert_owned = quote! {
        impl #impl_generics Insertable<#table_name::table> for #struct_name #ty_generics
            #where_clause
        {
            type Values = <(#(#direct_field_ty,)*) as Insertable<#table_name::table>>::Values;

            fn values(self) -> Self::Values {
                (#(#direct_field_assign,)*).values()
            }
        }
    };

    let insert_borrowed = if generate_borrowed_insert {
        quote! {
            impl #impl_generics Insertable<#table_name::table>
                for &'insert #struct_name #ty_generics
            #where_clause
            {
                type Values = <(#(#ref_field_ty,)*) as Insertable<#table_name::table>>::Values;

                fn values(self) -> Self::Values {
                    (#(#ref_field_assign,)*).values()
                }
            }
        }
    } else {
        quote! {}
    };

    Ok(wrap_in_dummy_mod(quote! {
        use diesel::insertable::Insertable;
        use diesel::query_builder::UndecoratedInsertRecord;
        use diesel::prelude::*;

        #insert_owned

        #insert_borrowed

        impl #impl_generics UndecoratedInsertRecord<#table_name::table>
                for #struct_name #ty_generics
            #where_clause
        {
        }
    }))
}

fn field_ty_embed(field: &Field, lifetime: Option<proc_macro2::TokenStream>) -> syn::Type {
    let field_ty = &field.ty;

    parse_quote!(#lifetime #field_ty)
}

fn field_expr_embed(field: &Field, lifetime: Option<proc_macro2::TokenStream>) -> syn::Expr {
    let field_access = field.name.access();

    parse_quote!(#lifetime self#field_access)
}

fn field_ty_serialize_as(field: &Field, table_name: &syn::Ident, ty: &syn::Type) -> syn::Type {
    let inner_ty = inner_of_option_ty(&ty);
    let column_name = field.column_name();

    parse_quote!(
        std::option::Option<diesel::dsl::Eq<
            #table_name::#column_name,
            #inner_ty,
        >>
    )
}

fn field_expr_serialize_as(field: &Field, table_name: &syn::Ident, ty: &syn::Type) -> syn::Expr {
    let field_access = field.name.access();
    let column_name = field.column_name();
    let column: syn::Expr = parse_quote!(#table_name::#column_name);

    if is_option_ty(&ty) {
        parse_quote!(self#field_access.map(|x| #column.eq(::std::convert::Into::<#ty>::into(x))))
    } else {
        parse_quote!(std::option::Option::Some(#column.eq(::std::convert::Into::<#ty>::into(self#field_access))))
    }
}

fn field_ty(
    field: &Field,
    table_name: &syn::Ident,
    lifetime: Option<proc_macro2::TokenStream>,
) -> syn::Type {
    let inner_ty = inner_of_option_ty(&field.ty);
    let column_name = field.column_name();

    parse_quote!(
        std::option::Option<diesel::dsl::Eq<
            #table_name::#column_name,
            #lifetime #inner_ty,
        >>
    )
}

fn field_expr(
    field: &Field,
    table_name: &syn::Ident,
    lifetime: Option<proc_macro2::TokenStream>,
) -> syn::Expr {
    let field_access = field.name.access();
    let column_name = field.column_name();
    let column: syn::Expr = parse_quote!(#table_name::#column_name);
    if is_option_ty(&field.ty) {
        if lifetime.is_some() {
            parse_quote!(self#field_access.as_ref().map(|x| #column.eq(x)))
        } else {
            parse_quote!(self#field_access.map(|x| #column.eq(x)))
        }
    } else {
        parse_quote!(std::option::Option::Some(#column.eq(#lifetime self#field_access)))
    }
}
