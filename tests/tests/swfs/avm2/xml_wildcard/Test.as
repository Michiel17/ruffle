package {
  import flash.display.Sprite;
  public class Test extends Sprite { }
}

var xml = <animals x="y" a="b">
    <animal id="1"><name>toto</name></animal>
    <animal id="2"><name>piggy</name></animal>
</animals>;

var attrs = xml.@*;
for each (var attr in attrs) {
  trace("attr: " + attr.name());
}

for each (var attr in xml.attribute("*")) {
  trace('attribute("*") attr: ' + attr.name());
}

var children = xml.*;
for each (var child in children) {
  trace("child.id: " + child.@id);
}

for each (var child in xml.child("*")) {
  trace('child("*") child.id: ' + child.@id);
}