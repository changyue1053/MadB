package org.cpf.kotlin.akinterop.generate
object ManagedRecordValues { fun validateString(text:String,name:String) { require(text.encodeToByteArray().size<=1024) } }
