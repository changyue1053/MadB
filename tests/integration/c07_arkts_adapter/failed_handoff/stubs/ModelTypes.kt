package com.madb.c07
class Snapshot(val value:Int,val text:String)
interface RuntimeDispatcher { fun onReady(); fun onValue(handle:String,snapshot:Snapshot); fun onClosed(handle:String,status:Int) }
