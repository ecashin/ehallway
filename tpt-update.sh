#! /bin/sh

mydir=`dirname "$0"`
app_hash=`ls "$mydir"/ui/dist/index-*.js | sed 's!\.js$!!' | sed 's!.*-!!'`
sed "s/@APPHASH@/$app_hash/g" \
    "$mydir"/api/templates/base.html.tera.elc-template \
    > "$mydir"/api/templates/base.html.tera
